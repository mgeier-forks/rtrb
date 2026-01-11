macro_rules! ring_buffer_instantiation {
    // Fixed-size header fields are recursively grouped into brackets,
    // only the last field (which is dynamically sized) remains outside.

    // Initial match. Start a bracket
    (
        $(#[$struct_attr:meta])*
        pub struct RingBuffer<T> {
            $(#[$next_field_attr:meta])*
            $vis:vis $next_field_name:ident: $next_field_type:ty,
            $($tail:tt)+
        }
    ) => {
        ring_buffer_instantiation! {
            $(#[$struct_attr])*
            pub struct RingBuffer<T> {
                [
                    $(#[$next_field_attr])*
                    $vis $next_field_name: $next_field_type,
                ]
                $($tail)+
            }
        }
    };

    (
        $(#[$struct_attr:meta])*
        pub struct RingBuffer<T> {
            [
                $($header_field:tt)*
            ]

            $(#[$next_field_attr:meta])*
            $vis:vis $next_field_name:ident: $next_field_type:ty,

            $($tail:tt)+
        }
    ) => {
        ring_buffer_instantiation! {
            $(#[$struct_attr])*
            pub struct RingBuffer<T> {
                [
                    $($header_field)*

                    $(#[$next_field_attr])*
                    $vis $next_field_name: $next_field_type,
                ]

                $($tail)+
            }
        }
    };

    // Final recursion: All fixed-size fields are already within the brackets.
    (
        $(#[$struct_attr:meta])*
        pub struct RingBuffer<T> {
            [
                $(
                    $(#[$field_attr:meta])*
                    $vis:vis $field_name:ident: $field_type:ty,
                )*
            ]

            $(#[$last_field_attr:meta])*
            $last_field_name:ident: $last_field_type:ty,
        }
    ) => {
        $(#[$struct_attr])*
        #[repr(C)]
        pub struct RingBuffer<T> {
            $(
                $(#[$field_attr])*
                $vis $field_name: $field_type,
            )*

            $(#[$last_field_attr])*
            $last_field_name: $last_field_type,
        }

        impl<T> RingBuffer<T> {
            /// Allocates memory and Default-initializes the fixed-size fields.
            ///
            /// The dynamically-sized part remains uninitialized.
            fn instantiate(capacity: usize) -> NonNull<Self> {
                use alloc::alloc::Layout;
                // Start with an empty layout ...
                let layout = Layout::new::<()>();
                // ... and add all fields from RingBuffer,
                // which must have #[repr(C)] (which we added above)!
                $(
                    let (layout, _) = layout
                        .extend(Layout::new::<$field_type>())
                        .unwrap();
                )*
                // After the fixed-size fields, we need space for the slots.
                let (layout, _) = layout
                    .extend(Layout::array::<T>(capacity).unwrap())
                    .unwrap();
                let layout = layout.pad_to_align();

                // SAFETY: `layout` has non-zero size.
                let ptr = unsafe { alloc::alloc::alloc(layout) };
                if ptr.is_null() {
                    alloc::alloc::handle_alloc_error(layout);
                }
                // Create a (fat) pointer to a slice ...
                let ptr: *mut [T] = core::ptr::slice_from_raw_parts_mut(ptr.cast(), capacity);
                // ... and coerce it into our own dynamically sized type:
                let ptr = ptr as *mut Self;
                // SAFETY: Offsets and types of fields are correct.
                unsafe {
                $(
                    (&raw mut (*ptr).$field_name).write(Default::default());
                )*
                }
                // SAFETY: Null check has been done above
                unsafe { NonNull::new_unchecked(ptr) }
            }
        }
    };
}
