pub mod external_editor;
pub mod fuzzy;

#[macro_export]
macro_rules! iterable_enum {
    ($visibility:vis, $name:ident, $($member:tt),*) => {

        #[derive(Copy, Clone, Debug)]
        $visibility enum $name {$($member),*}
        impl $name {
            pub fn iter() -> impl Iterator<Item = $name> {
                    const VARIANTS: &[$name] = &[$($name::$member),*];
                    VARIANTS.iter().copied()
                }
        }
    };
    ($name:ident, $($member:tt),*) => {
        iterable_enum!(, $name, $($member),*)
    };
}
