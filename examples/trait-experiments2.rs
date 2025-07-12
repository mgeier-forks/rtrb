use std::marker::PhantomData;

pub trait Capacity {
    fn capacity(&self) -> usize;
}

pub trait Calc {
    const POW2: bool;
    //const INIT: Self;
}

pub trait IndexCalculation {
    fn increment1(&self, pos: usize) -> usize;
}

// work-around for missing const fn in traits feature
const fn update_capacity<C: Calc>(capacity: usize) -> usize {
    if C::POW2 {
        capacity.next_power_of_two()
    } else {
        capacity
    }
}

struct DoubleRange;

impl Calc for DoubleRange {
    const POW2: bool = false;
}

struct DoubleRangePowerOfTwo;

impl Calc for DoubleRangePowerOfTwo {
    const POW2: bool = true;
}

pub struct ConstCapacity<const N: usize>;
impl<const N: usize> Capacity for ConstCapacity<N> {
    fn capacity(&self) -> usize {
        N
    }
}
pub struct DynamicCapacity(usize);
impl DynamicCapacity {
    pub fn new(capacity: usize) -> Self {
        Self(capacity)
    }
}
impl Capacity for DynamicCapacity {
    fn capacity(&self) -> usize {
        self.0
    }
}
pub struct Calculator<C: Calc, H: Capacity> {
    _phantom: PhantomData<C>,
    c: H,
}

impl<C: Calc, H: Capacity> Calculator<C, H> {
    pub const fn new(c: H) -> Self {
        Self {
            _phantom: PhantomData,
            c,
        }
    }
}

impl<C: Calc, H: Capacity> Capacity for Calculator<C, H> {
    fn capacity(&self) -> usize {
        self.c.capacity()
    }
}

impl<H: Capacity> IndexCalculation for Calculator<DoubleRange, H>
where
    Self: Capacity,
{
    #[inline]
    fn increment1(&self, pos: usize) -> usize {
        if pos < 2 * self.capacity() - 1 {
            pos + 1
        } else {
            0
        }
    }
}

impl<H: Capacity> IndexCalculation for Calculator<DoubleRangePowerOfTwo, H> {
    #[inline]
    fn increment1(&self, pos: usize) -> usize {
        pos.wrapping_add(1)
    }
}

pub trait Storage {
    type Calculator: IndexCalculation + Capacity;

    fn calc(&self) -> &Self::Calculator;

    fn drop_all_elements(&self, value: usize) -> usize {
        self.calc().increment1(value)
    }
}

pub struct DynamicStorage<C: Calc> {
    calc: Calculator<C, DynamicCapacity>,
}

impl<C: Calc> DynamicStorage<C> {
    fn new(capacity: usize) -> Self {
        let capacity = update_capacity::<C>(capacity);
        Self {
            calc: Calculator::new(DynamicCapacity::new(capacity)),
        }
    }
}

impl<C: Calc> Storage for DynamicStorage<C>
where
    Calculator<C, DynamicCapacity>: IndexCalculation,
{
    type Calculator = Calculator<C, DynamicCapacity>;

    fn calc(&self) -> &Self::Calculator {
        &self.calc
    }
}

pub struct ArrayStorage<const N: usize, C: Calc> {
    calc: Calculator<C, ConstCapacity<N>>,
}

impl<const N: usize, C: Calc> ArrayStorage<N, C> {
    const fn new() -> Self {
        const {
            assert!(
                update_capacity::<C>(N) == N,
                "`capacity` must be a power of two"
            );
        }
        Self {
            calc: Calculator::new(ConstCapacity),
        }
    }
}

impl<const N: usize, C: Calc> Storage for ArrayStorage<N, C>
where
    Calculator<C, ConstCapacity<N>>: IndexCalculation,
{
    type Calculator = Calculator<C, ConstCapacity<N>>;

    fn calc(&self) -> &Self::Calculator {
        &self.calc
    }
}

struct Producer<S: Storage> {
    storage: S,
}

impl<S: Storage> Producer<S> {
    fn new(storage: S) -> Self {
        Self { storage }
    }
    fn push(&self, value: usize) -> usize {
        self.storage.calc().increment1(value)
    }
    fn capacity(&self) -> usize {
        self.storage.calc().capacity()
    }
}

fn main() {
    let p = Producer::new(DynamicStorage::<DoubleRange>::new(14));
    let n = 27;
    println!(
        "increment {} (capacity {}) -> {}",
        n,
        p.capacity(),
        p.push(n)
    );

    let p = Producer::new(ArrayStorage::<16, DoubleRangePowerOfTwo>::new());
    println!(
        "increment {} (capacity {}) -> {}",
        n,
        p.capacity(),
        p.push(n)
    );
}
