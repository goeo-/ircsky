pub trait ParamMaybe {
    fn param_maybe<S>(self, param: Option<S>) -> irc_rust::builder::Builder
    where
        S: ToString;
}

impl ParamMaybe for irc_rust::builder::Builder {
    fn param_maybe<S>(self, param: Option<S>) -> irc_rust::builder::Builder
    where
        S: ToString,
    {
        match param {
            Some(param) => self.param(param),
            None => self,
        }
    }
}
