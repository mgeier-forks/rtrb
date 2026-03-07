macro_rules! dst_ring_buffer_instantiation {
    // NB: The = character in =[ was added to avoid a parsing ambiguity.

    // Fixed-size header fields are recursively moved into the brackets,
    // only the last field (which is dynamically sized) remains outside.
    (
        $(#[$struct_attr:meta])*
        pub struct RingBuffer<T> {
            $(=[
                $($already_bracketed:tt)*
            ])?

            $(#[$next_field_attr:meta])*
            $next_field_vis:vis $next_field_name:ident: $next_field_type:ty,

            $($remaining_fields:tt)+
        }
    ) => {
        dst_ring_buffer_instantiation! {
            $(#[$struct_attr])*
            pub struct RingBuffer<T> {
                =[
                    $($($already_bracketed)*)?

                    $(#[$next_field_attr])*
                    $next_field_vis $next_field_name: $next_field_type,
                ]

                $($remaining_fields)+
            }
        }
    };

    // Base case: Only one field (the dynamically-sized one) remains outside the brackets.
    (
        $(#[$struct_attr:meta])*
        pub struct RingBuffer<T> {
            =[
                $(
                    $(#[$field_attr:meta])*
                    $field_vis:vis $field_name:ident: $field_type:ty,
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
                $field_vis $field_name: $field_type,
            )*

            $(#[$last_field_attr])*
            $last_field_name: $last_field_type,
        }

        use alloc::alloc::Layout;

        impl<T> RingBuffer<T> {
            /// Calculate memory layout using the given `capacity`.
            fn layout_helper(capacity: usize) -> Layout {
                // TODO: check for power-of-two capacity?

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
                layout.pad_to_align()
            }

            fn coerce(ptr: *mut u8, capacity: usize) -> *mut Self {
                // TODO: check for power-of-two capacity?

                // Create a (fat) pointer to a slice ...
                let ptr: *mut [T] = core::ptr::slice_from_raw_parts_mut(ptr.cast(), capacity);
                // ... and coerce it into our own dynamically sized type:
                ptr as *mut Self
            }

            // SAFETY: Pointer must be valid and no reference exists.
            unsafe fn default_initialize(ptr: *mut Self) {
                // SAFETY: Offsets and types of fields are correct.
                unsafe {
                $(
                    core::ptr::addr_of_mut!((*ptr).$field_name).write(Default::default());
                    // With MSRV 1.82, this can be used instead:
                    //(&raw mut (*ptr).$field_name).write(Default::default());
                )*
                }
            }

            /// Allocates memory and Default-initializes the fixed-size fields.
            ///
            /// The dynamically-sized part remains uninitialized.
            fn instantiate(capacity: usize) -> Box<Self> {
                let layout = Self::layout_helper(capacity);
                // SAFETY: `layout` has non-zero size.
                let ptr = unsafe { alloc::alloc::alloc(layout) };
                if ptr.is_null() {
                    alloc::alloc::handle_alloc_error(layout);
                }
                let ptr = Self::coerce(ptr, capacity);
                // SAFETY: The allocation has been created according to `layout()`
                // it will live long enough and no other reference exists.
                unsafe {
                    Self::default_initialize(ptr);
                }
                // SAFETY: The memory has been allocated in a compatible way
                // and this is the only time we create a `Box` from it.
                unsafe { Box::from_raw(ptr) }
            }
        }
    };
}
