#[derive(Debug)]
pub struct KvsGetArgs {
    pub key: String,
}

#[derive(Debug)]
pub struct KvsSetArgs {
    pub key: String,
    pub value: String,
}
