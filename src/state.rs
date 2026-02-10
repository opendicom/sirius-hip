use std::sync::Arc;

use crate::persistence::JwtRepository;


pub struct AppState {
    pub jwt_repo: Option<Arc<dyn JwtRepository>>,
}