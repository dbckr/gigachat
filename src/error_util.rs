use tracing::error;
use std::fmt::Debug;

// yoinking log_err ( https://github.com/DesmondWillowbrook/rs-log_err) to use these methods with tracing crate
pub trait LogErrResult<T, E: Debug> {
    fn log_unwrap (self) -> T;
    fn log_expect (self, msg: &str) -> T;
    fn log_inspect_err (self) -> Self;
}

pub trait LogErrOption<T> {
    fn log_unwrap (self) -> T;
    fn log_expect (self, msg: &str) -> T;
}

impl<T> LogErrOption<T> for Option<T> {

    /**
    `unwrap`s the `Option`, and outputs error message (in exact same style as `unwrap`) through `error!` as well.
    */

    fn log_unwrap (self) -> T {
        match self {
            Some (n) => n,
            None => {
                error!("called `Option::unwrap()` on a `None` value");
                self.unwrap()
            }
        }
    }

    /**
    `expect`s the `Option`, and outputs error message (in exact same style as `expect`) through `error!` as well.
    */

    fn log_expect (self, msg: &str) -> T {
        match self {
            Some (n) => n,
            None => {
                error!("{}", msg);
                self.expect(msg)
            },
        }
    }
}

impl<T, E: Debug> LogErrResult<T, E> for Result<T, E> {

    /**
    `unwrap`s the `Result`, and outputs error message (in exact same style as `unwrap`) through `error!` as well.
    */

    fn log_unwrap (self) -> T {
        self.map_err(|e| {error!("called `Result::unwrap()` on an `Err` value: {:?}", e); e}).unwrap()
    }

    /**
    `expect`s the `Result`, and outputs error message (in exact same style as `expect`) through `error!` as well.
    */

    fn log_expect (self, msg: &str) -> T {
        self.map_err(|e| {error!("{}: {:?}", msg, e); e}).expect(msg)
    }

    fn log_inspect_err (self) -> Self {
      self.inspect_err(|e| { error!("{:?}", e); })
  }
}