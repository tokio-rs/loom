pub(crate) use cfg::Backtrace;

#[cfg(feature = "backtrace")]
mod cfg {
    use std::fmt;

    #[derive(Debug)]
    pub(crate) struct Backtrace(backtrace::Backtrace);

    impl Backtrace {
        pub(crate) fn capture() -> Backtrace {
            Backtrace(backtrace::Backtrace::new())
        }
    }

    impl fmt::Display for Backtrace {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
            // TODO: Add some sort of filter to get rid of loom internals
            write!(fmt, "{:?}", self.0)
        }
    }
}

#[cfg(not(feature = "backtrace"))]
mod cfg {
    use std::fmt;

    #[derive(Debug)]
    pub(crate) struct Backtrace;

    impl Backtrace {
        pub(crate) fn capture() -> Backtrace {
            panic!("enable `backtrace` feature flag");
        }
    }

    impl fmt::Display for Backtrace {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(fmt, "[enable `backtrace` feature for backtrace capture]")
        }
    }
}
