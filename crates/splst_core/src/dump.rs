use crate::bus::AddrUnit;

/// Printer trait to dump system info.
pub trait Dumper {
    fn dump_addr_unit(&mut self, label: &'static str, addr: impl AddrUnit);
    
    fn dump_string(&mut self, label: &'static str, string: String);
}

/// Macro to format the dump output.
#[macro_export]
macro_rules! dump {
    ($dumper:expr, $label:expr, $($args:tt)+) => (
        $dumper.dump_string($label, format!($($args)+))
    )
}
