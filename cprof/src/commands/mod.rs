pub mod root;
pub(crate) mod support;
pub mod target;

// Missing #[command(...)] on a command entry point fails compilation here.
macro_rules! command_modules {
    ($($name:ident),* $(,)?) => {
        $(
            pub mod $name;
            const _: () = $name::COMMAND_MARKER;
        )*
    };
}

pub(crate) use command_modules;
