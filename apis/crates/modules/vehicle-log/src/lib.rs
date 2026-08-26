//
//  cp-vehicle-log
//  lib.rs
//
//  Created by Ngonidzashe Mangudya on 2026/08/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//
//  Vehicle Daily Log module: day-to-day trip sheets against vehicles and
//  drivers owned by cp-fleet. Deliberately a separate module from Fleet
//  Management — the daily-ops log has its own workflow (draft -> submitted
//  -> approved) distinct from the vehicle/driver registry itself.

pub mod dtos;
pub mod models;
pub mod ops;
pub mod routes;
