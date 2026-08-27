//! Erases typed capability handlers only after input parsing remains available.
//!
//! Each adapter deserializes into its own deny-unknown-fields input type before
//! scope authorization or execution, so raw provider JSON never reaches a domain service.

use std::any::Any;

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::{
    descriptor::CapabilityDescriptor,
    types::{AuthorizedCapabilityContext, CapabilityExecutionError, CapabilityScope},
};

#[async_trait]
pub trait Capability: Send + Sync + 'static {
    type Input: DeserializeOwned + Send + Sync + 'static;
    type Output: Serialize + Send + 'static;

    fn descriptor(&self) -> &CapabilityDescriptor;

    fn scope(&self, input: &Self::Input) -> CapabilityScope;

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError>;
}

pub(crate) struct ParsedCapabilityInput(Box<dyn Any + Send + Sync>);

#[derive(Debug)]
pub(crate) enum ErasedCapabilityError {
    InvalidInput,
    Contract,
    Execution(CapabilityExecutionError),
}

#[async_trait]
pub(crate) trait ErasedCapability: Send + Sync {
    fn descriptor(&self) -> &CapabilityDescriptor;

    fn parse_input(&self, input: Value) -> Result<ParsedCapabilityInput, ErasedCapabilityError>;

    fn scope(
        &self,
        input: &ParsedCapabilityInput,
    ) -> Result<CapabilityScope, ErasedCapabilityError>;

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: ParsedCapabilityInput,
    ) -> Result<Value, ErasedCapabilityError>;
}

pub(crate) struct CapabilityAdapter<C> {
    capability: C,
}

impl<C> CapabilityAdapter<C>
where
    C: Capability,
{
    pub(crate) const fn new(capability: C) -> Self {
        Self { capability }
    }
}

#[async_trait]
impl<C> ErasedCapability for CapabilityAdapter<C>
where
    C: Capability,
{
    fn descriptor(&self) -> &CapabilityDescriptor {
        self.capability.descriptor()
    }

    fn parse_input(&self, input: Value) -> Result<ParsedCapabilityInput, ErasedCapabilityError> {
        serde_json::from_value::<C::Input>(input)
            .map(|input| ParsedCapabilityInput(Box::new(input)))
            .map_err(|_| ErasedCapabilityError::InvalidInput)
    }

    fn scope(
        &self,
        input: &ParsedCapabilityInput,
    ) -> Result<CapabilityScope, ErasedCapabilityError> {
        let Some(input) = input.0.downcast_ref::<C::Input>() else {
            return Err(ErasedCapabilityError::Contract);
        };
        Ok(self.capability.scope(input))
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: ParsedCapabilityInput,
    ) -> Result<Value, ErasedCapabilityError> {
        let Ok(input) = input.0.downcast::<C::Input>() else {
            return Err(ErasedCapabilityError::Contract);
        };
        let output = self
            .capability
            .execute(context, *input)
            .await
            .map_err(ErasedCapabilityError::Execution)?;
        serde_json::to_value(output).map_err(|_| ErasedCapabilityError::Contract)
    }
}
