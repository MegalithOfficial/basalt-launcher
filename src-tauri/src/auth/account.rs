use serde::Serialize;

#[derive(Clone)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub mc_access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountView {
    pub id: String,
    pub name: String,
    pub active: bool,
}
