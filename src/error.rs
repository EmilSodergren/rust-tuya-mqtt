use thiserror::Error;

#[derive(Error, Debug)]
pub enum ErrorKind {
    #[error("Topic too short")]
    TopicTooShort,
}
