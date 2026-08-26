//
//  cp-fleet
//  lib.rs
//
//  Created by Ngonidzashe Mangudya on 2026/08/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//
//  Fleet Management module: the vehicle and driver registry every other
//  fleet-adjacent module (e.g. cp-vehicle-log) reads from.

pub mod dtos;
pub mod models;
pub mod ops;
pub mod routes;

pub use models::{Driver, Vehicle};
