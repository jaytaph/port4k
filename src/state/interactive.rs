#[derive(Debug, Clone)]
pub enum InteractiveState {
    None,
    LoginAskUsername,
    LoginAskPassword { username: String },
    Register(RegisterState),
}

#[derive(Debug, Clone, Default)]
pub struct RegisterState {
    pub username: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
}
