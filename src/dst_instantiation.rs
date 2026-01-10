macro_rules! ring_buffer_instantiation {
    // Fixed-size header fields are recursively grouped into brackets,
    // only the last field (which is dynamically sized) remains outside.
    (
        $(#[$struct_attr:meta])*
        pub struct RingBuffer<T> {
            $(
                [
                    $($header_field:tt)*
                ]
            )?

            $(#[$next_field_attr:meta])*
            pub(super) $next_field_name:ident: $next_field_type:ty,

            $($tail:tt)+
        }
    ) => {
        ring_buffer_instantiation! {
            $(#[$struct_attr])*
            pub struct RingBuffer<T> {
                [
                    $(
                        $($header_field)*
                    )?

                    $(#[$next_field_attr])*
                    pub(super) $next_field_name: $next_field_type,
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
                    pub(super) $field_name:ident: $field_type:ty,
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
                pub(super) $field_name: $field_type,
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
                    // We abuse $field_name as variable name for the field offset:
                    let (layout, $field_name) = layout
                        .extend(Layout::new::<$field_type>())
                        .unwrap();
                )*
                // After the fixed-size fields, we need space for the slots.
                let (layout, _) = layout
                    .extend(Layout::array::<T>(capacity).unwrap())
                    .unwrap();
                let layout = layout.pad_to_align();

                unsafe {
                    let ptr = alloc::alloc::alloc(layout);
                    if ptr.is_null() {
                        alloc::alloc::handle_alloc_error(layout);
                    }
                    $(
                        &raw mut (*ptr).$field_name.write(Default::default());
                        //core::ptr::addr_of_mut!((*ptr).$field_name).write(Default::default());
                    )*
                    // Create a (fat) pointer to a slice ...
                    let ptr: *mut [T] = core::ptr::slice_from_raw_parts_mut(ptr.cast(), capacity);
                    // ... and coerce it into our own dynamically sized type:
                    let ptr = ptr as *mut Self;
                    // Safety: Null check has been done above
                    NonNull::new_unchecked(ptr)
                }
            }
        }
    };
}
