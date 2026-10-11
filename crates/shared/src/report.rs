use std::error::Error;

pub fn report(error: &dyn Error) {
    eprintln!("error: {error}");
    let mut cause = error.source();
    while let Some(inner) = cause {
        eprintln!("  caused by: {inner}");
        cause = inner.source();
    }
}
