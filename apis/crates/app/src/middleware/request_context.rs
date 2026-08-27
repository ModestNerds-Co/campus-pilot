//! Creates server-owned request IDs and propagates valid correlation IDs.

use std::{
    future::{Ready, ready},
    rc::Rc,
};

use actix_web::{
    Error, HttpMessage,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::header::{HeaderName, HeaderValue},
};
use cp_audit::{CORRELATION_ID_HEADER, REQUEST_ID_HEADER, RequestContext};
use futures_util::future::LocalBoxFuture;
use uuid::Uuid;

pub struct RequestContextMiddleware;

impl<S, B> Transform<S, ServiceRequest> for RequestContextMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RequestContextMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestContextMiddlewareService {
            service: Rc::new(service),
        }))
    }
}

pub struct RequestContextMiddlewareService<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for RequestContextMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, request: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);
        let incoming_correlation_id = request
            .headers()
            .get(CORRELATION_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| Uuid::parse_str(value).ok());
        let request_context = RequestContext::generate(incoming_correlation_id);
        request.extensions_mut().insert(request_context);

        Box::pin(async move {
            let mut response = service.call(request).await?;
            let request_header = HeaderValue::from_str(&request_context.request_id().to_string())
                .map_err(actix_web::error::ErrorInternalServerError)?;
            let correlation_header =
                HeaderValue::from_str(&request_context.correlation_id().to_string())
                    .map_err(actix_web::error::ErrorInternalServerError)?;

            response
                .headers_mut()
                .insert(HeaderName::from_static(REQUEST_ID_HEADER), request_header);
            response.headers_mut().insert(
                HeaderName::from_static(CORRELATION_ID_HEADER),
                correlation_header,
            );
            Ok(response)
        })
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{App, HttpResponse, test, web};
    use cp_audit::{CORRELATION_ID_HEADER, REQUEST_ID_HEADER, RequestContext};
    use uuid::Uuid;

    use super::RequestContextMiddleware;

    async fn context_handler(context: web::ReqData<RequestContext>) -> HttpResponse {
        HttpResponse::Ok().body(context.request_id().to_string())
    }

    #[actix_web::test]
    async fn generates_request_and_correlation_headers() {
        let app = test::init_service(
            App::new()
                .wrap(RequestContextMiddleware)
                .route("/", web::get().to(context_handler)),
        )
        .await;
        let response =
            test::call_service(&app, test::TestRequest::get().uri("/").to_request()).await;

        let request_id = response
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| Uuid::parse_str(value).ok())
            .unwrap_or_else(|| unreachable!());
        let correlation_id = response
            .headers()
            .get(CORRELATION_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| Uuid::parse_str(value).ok())
            .unwrap_or_else(|| unreachable!());

        assert_eq!(request_id, correlation_id);
    }

    #[actix_web::test]
    async fn propagates_only_a_valid_correlation_id() {
        let app = test::init_service(
            App::new()
                .wrap(RequestContextMiddleware)
                .route("/", web::get().to(context_handler)),
        )
        .await;
        let incoming = Uuid::new_v4();
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/")
                .insert_header((CORRELATION_ID_HEADER, incoming.to_string()))
                .to_request(),
        )
        .await;

        let request_id = response
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| Uuid::parse_str(value).ok())
            .unwrap_or_else(|| unreachable!());
        let correlation_id = response
            .headers()
            .get(CORRELATION_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| Uuid::parse_str(value).ok())
            .unwrap_or_else(|| unreachable!());

        assert_ne!(request_id, incoming);
        assert_eq!(correlation_id, incoming);

        let invalid_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/")
                .insert_header((CORRELATION_ID_HEADER, "not-a-uuid"))
                .to_request(),
        )
        .await;
        assert_eq!(
            invalid_response.headers().get(REQUEST_ID_HEADER),
            invalid_response.headers().get(CORRELATION_ID_HEADER)
        );
    }
}
