//! POST /auth/oauth/exchange

use crate::modules::oauth::{dto::ExchangeDto, services::OAuthService};
use crate::{
    common::{error::AppResult, extractors::ValidatedJson},
    modules::auth::dto::TokenResponse,
};
use axum::{Json, extract::State};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/oauth/exchange",
    request_body = ExchangeDto,
    responses(
        (status = 200, description = "Access token for the completed sign-in", body = TokenResponse),
        (status = 400, description = "Code is invalid, expired or already used"),
    ),
    tag = "oauth"
)]
pub(crate) async fn exchange(
    State(oauth): State<Arc<OAuthService>>,
    ValidatedJson(dto): ValidatedJson<ExchangeDto>,
) -> AppResult<Json<TokenResponse>> {
    Ok(Json(oauth.exchange(&dto.code).await?))
}
