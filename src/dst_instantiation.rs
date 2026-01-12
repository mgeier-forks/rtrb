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

        use alloc::alloc::Layout;

        impl<T> RingBuffer<T> {
            /// Calculate memory layout using the given `capacity`.
            pub fn layout(capacity: usize) -> Layout {
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

            /// Creates a `RingBuffer` at the given memory address.
            ///
            /// # Safety
            ///
            /// The provided memory allocation must have the required size and alignment
            /// (see [`RingBuffer::layout()`]) and it must exist at least as long
            /// as the returned reference.
            pub unsafe fn new_at<'a>(ptr: *mut u8, capacity: usize) -> &'a Self {
                // TODO: check for power-of-two capacity?

                let ptr = Self::coerce(ptr, capacity);
                // SAFETY: Offsets and types of fields are correct.
                unsafe {
                $(
                    core::ptr::addr_of_mut!((*ptr).$field_name).write(Default::default());
                    // With MSRV 1.82, this can be used instead:
                    //(&raw mut (*ptr).$field_name).write(Default::default());
                )*
                }
                // SAFETY: `ptr` is non-null and object is fully initialized.
                unsafe { &*ptr }
            }

            /// Provides access to an existing `RingBuffer` at a given memory address.
            ///
            /// [`RingBuffer::new_at()`] creates a `RingBuffer` at some
            /// (allocated but uninitialized) memory location.
            /// `RingBuffer::from_raw_parts()` provides access to an already
            /// existing `RingBuffer` at the given address. Both are extremely unsafe!
            ///
            /// # Safety
            ///
            /// The provided memory must have been initialized by [`RingBuffer::new_at()`]
            /// and the memory allocation must exist at least as long as the returned reference.
            pub unsafe fn from_raw_parts<'a>(ptr: *mut u8, capacity: usize) -> &'a Self {
                // TODO: check for power-of-two capacity?

                let ptr = Self::coerce(ptr, capacity);
                // SAFETY: `ptr` must be non-null and object must be fully initialized.
                unsafe { &*ptr }
            }

            fn coerce(ptr: *mut u8, capacity: usize) -> *mut Self {
                // TODO: check for power-of-two capacity?

                // Create a (fat) pointer to a slice ...
                let ptr: *mut [T] = core::ptr::slice_from_raw_parts_mut(ptr.cast(), capacity);
                // ... and coerce it into our own dynamically sized type:
                ptr as *mut Self
            }

            /// Allocates memory and Default-initializes the fixed-size fields.
            ///
            /// The dynamically-sized part remains uninitialized.
            fn instantiate(capacity: usize) -> Box<Self> {
                let layout = Self::layout(capacity);
                // SAFETY: `layout` has non-zero size.
                let ptr = unsafe { alloc::alloc::alloc(layout) };
                if ptr.is_null() {
                    alloc::alloc::handle_alloc_error(layout);
                }
                // SAFETY: The allocation has been created according to `layout()`
                // and it will live long enough.
                let rb = unsafe { Self::new_at(ptr, capacity) };
                // SAFETY: The memory has been allocated in a compatible way
                // and this is the only time we create a `Box` from it.
                unsafe { Box::from_raw(rb as *const _ as *mut _) }
            }
        }
    };
}
