macro_rules! dst_ring_buffer_instantiation {
    // Fixed-size fields are recursively moved into the brackets,
    // only the dynamically sized `slots` remain outside.
    (
        $(#[$struct_attr:meta])*
        pub struct RingBuffer<T> {
            $([
                $($already_bracketed:tt)*
            ])?
            $next_field_name:ident: $next_field_type:ty,
            $($remaining_fields:tt)+
        }

        $($more_stuff:tt)*
    ) => {
        dst_ring_buffer_instantiation! {
            $(#[$struct_attr])*
            pub struct RingBuffer<T> {
                [
                    $($($already_bracketed)*)?
                    $next_field_name: $next_field_type,
                ]
                $($remaining_fields)+
            }

            $($more_stuff)*
        }
    };

    // All fixed-size fields are in the bracket, handling the ArrayRingBuffer, if given.
    (
        $(#[$struct_attr:meta])*
        pub struct RingBuffer<T> {
            [
                $($field_name:ident: $field_type:ty,)*
            ]
            slots: UnsafeCell<[MaybeUninit<T>]>,
        }

        $(#[$array_struct_attr:meta])*
        pub struct ArrayRingBuffer<T, const N: usize>;
    ) => {
        dst_ring_buffer_instantiation! {
            $(#[$struct_attr])*
            pub struct RingBuffer<T> {
                [
                    $($field_name: $field_type,)*
                ]
                slots: UnsafeCell<[MaybeUninit<T>]>,
            }
        }

        $(#[$array_struct_attr])*
        #[repr(C)]
        pub struct ArrayRingBuffer<T, const N: usize> {
            $($field_name: $field_type,)*
            slots: UnsafeCell<[MaybeUninit<T>; N]>,
        }

        impl<T, const N: usize> ArrayRingBuffer<T, N> {
            /// Creates an ArrayRingBuffer with the given fixed-size fields.
            ///
            /// The `slots` remain uninitialized.
            const fn from_sized_fields(fields: Fields) -> Self {
                let Fields { $($field_name,)* } = fields;
                Self {
                    $($field_name,)*
                    slots: UnsafeCell::new([const { MaybeUninit::uninit() }; N]),
                }
            }

            /// Returns a reference to the dynamically sized [`RingBuffer`].
            ///
            /// In most cases, calling this is not necessary,
            /// because `ArrayRingBuffer` automatically dereferences to `RingBuffer`.
            /// It might be useful in `const` contexts, though.
            pub const fn as_unsized(&self) -> &RingBuffer<T> {
                let ptr: *const Self = self;
                // Turn the self-pointer into a (fat) pointer to a slice ...
                let ptr: *const [T] = core::ptr::slice_from_raw_parts(ptr.cast(), N);
                // ... and then into a pointer to the dynamically sized RingBuffer.
                // SAFETY: Both structs are #[repr(C)] and have identical fields
                // (both guaranteed by the macro), except for `slots`,
                // which is an array or slice, respectively.
                // The slice length is set to `N` here, so both have the same memory layout.
                unsafe { &*(ptr as *const RingBuffer<T>) }
            }

            /// Like `as_unsized()`, but mutable.
            const fn as_unsized_mut(&mut self) -> &mut RingBuffer<T> {
                let ptr: *mut Self = self;
                let ptr: *mut [T] = core::ptr::slice_from_raw_parts_mut(ptr.cast(), N);
                // SAFETY: See as_unsized().
                unsafe { &mut *(ptr as *mut RingBuffer<T>) }
            }
        }
    };

    // Base case: All fixed-size fields are in the bracket, no ArrayRingBuffer.
    (
        $(#[$struct_attr:meta])*
        pub struct RingBuffer<T> {
            [
                $($field_name:ident: $field_type:ty,)*
            ]
            slots: UnsafeCell<[MaybeUninit<T>]>,
        }
    ) => {
        $(#[$struct_attr])*
        #[repr(C)]
        pub struct RingBuffer<T> {
            $($field_name: $field_type,)*
            slots: UnsafeCell<[MaybeUninit<T>]>,
        }

        /// All fixed-size fields of RingBuffer, only used for initialization.
        struct Fields {
            $($field_name: $field_type,)*
        }

        impl<T> RingBuffer<T> {
            /// Calculate memory layout using the given `capacity`.
            fn layout_helper(capacity: usize) -> core::alloc::Layout {
                // Start with an empty layout ...
                let layout = core::alloc::Layout::new::<()>();
                // ... and add all fields from RingBuffer,
                // which must have #[repr(C)] (which we added above)!
                $(
                    let (layout, _) = layout
                        .extend(core::alloc::Layout::new::<$field_type>())
                        .unwrap();
                )*
                // After the fixed-size fields, we need space for the slots.
                let (layout, _) = layout
                    .extend(core::alloc::Layout::array::<T>(capacity).unwrap())
                    .unwrap();
                layout.pad_to_align()
            }

            // This function is safe, but using the returned pointer is unsafe.
            fn coerce(ptr: *mut u8, capacity: usize) -> *mut Self {
                // Create a (fat) pointer to a slice ...
                let ptr: *mut [T] = core::ptr::slice_from_raw_parts_mut(ptr.cast(), capacity);
                // ... and coerce it into our own dynamically sized type:
                ptr as *mut Self
            }

            /// Initializes a RingBuffer at the given memory location.
            ///
            /// # Safety
            ///
            /// Pointer must be valid for writes, have the size and alignment given by
            /// `layout_helper()` (with the same `capacity`) and no reference may exist.
            unsafe fn from_sized_fields(ptr: *mut u8, capacity: usize, fields: Fields) -> *mut Self {
                let ptr = Self::coerce(ptr, capacity);
                let Fields { $($field_name,)* } = fields;
                // SAFETY: Offsets and types of fields are correct.
                unsafe {
                $(
                    core::ptr::addr_of_mut!((*ptr).$field_name).write($field_name);
                    // With MSRV 1.82, this can be used instead:
                    //(&raw mut (*ptr).$field_name).write($field_name);
                )*
                }
                ptr
            }
        }
    };
}
