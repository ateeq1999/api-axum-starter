use serde::Deserialize;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct UploadMediaQuery {
    /// The file's original name, kept for display and as the download name.
    #[validate(length(min = 1, max = 255))]
    pub filename: Option<String>,
}
