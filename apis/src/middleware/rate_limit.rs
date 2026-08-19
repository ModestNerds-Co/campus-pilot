//
//  campus-pilot-apis
//  rate_limit.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/02.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use actix_governor::{Governor, GovernorConfigBuilder, PeerIpKeyExtractor};

/// Create a rate limiter for authentication endpoints
/// Limits to 5 requests per 15 minutes per IP
pub fn auth_rate_limiter() -> Governor<PeerIpKeyExtractor> {
    Governor::new(
        &GovernorConfigBuilder::default()
            .per_second(5)
            .burst_size(5)
            .finish()
            .unwrap(),
    )
}

/// Create a rate limiter for token refresh endpoint
/// Limits to 10 requests per 15 minutes per IP
pub fn refresh_rate_limiter() -> Governor<PeerIpKeyExtractor> {
    Governor::new(
        &GovernorConfigBuilder::default()
            .per_second(10)
            .burst_size(10)
            .finish()
            .unwrap(),
    )
}
