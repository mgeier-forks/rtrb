use std::marker::PhantomData;

pub trait Capacity {
    fn capacity(&self) -> usize;
}

pub trait Calc {
    const POW2: bool;
    //const INIT: Self;
}

//trait IndexCalculation: Capacity {
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

struct DoubleLength;

impl Calc for DoubleLength {
    const POW2: bool = false;
    //const INIT: Self = Self;
}

struct DoubleLengthPowerOfTwo;

impl Calc for DoubleLengthPowerOfTwo {
    const POW2: bool = true;
    //const INIT: Self = Self;
}

pub trait CapacityHolder {}
pub struct ConstCapacity<const N: usize>;
impl<const N: usize> CapacityHolder for ConstCapacity<N> {}
impl<const N: usize, C: Calc> Capacity for CalcHolder<C, ConstCapacity<N>> {
    fn capacity(&self) -> usize {
        N
    }
}
pub struct DynamicCapacity {
    capacity: usize,
}
impl CapacityHolder for DynamicCapacity {}
impl<C: Calc> Capacity for CalcHolder<C, DynamicCapacity> {
    fn capacity(&self) -> usize {
        self.cap.capacity
    }
}
pub struct CalcHolder<C: Calc, H: CapacityHolder> {
    _phantom: PhantomData<C>,
    cap: H,
}

impl<H: CapacityHolder> IndexCalculation for CalcHolder<DoubleLength, H>
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

impl<H: CapacityHolder> IndexCalculation for CalcHolder<DoubleLengthPowerOfTwo, H> {
    #[inline]
    fn increment1(&self, pos: usize) -> usize {
        pos.wrapping_add(1)
    }
}

pub trait Storage {
    type Calc: IndexCalculation + Capacity;

    fn calc(&self) -> &Self::Calc;

    fn drop_all_elements(&self, value: usize) -> usize {
        self.calc().increment1(value)
    }
}

pub struct DynamicStorage<C: Calc> {
    calc: CalcHolder<C, DynamicCapacity>,
}

impl<C: Calc> DynamicStorage<C> {
    fn new(capacity: usize) -> Self {
        let capacity = update_capacity::<C>(capacity);
        Self {
            // TODO: use CalcHolder::new()?
            calc: CalcHolder {
                cap: DynamicCapacity { capacity },
                _phantom: PhantomData,
            },
        }
    }
}

impl<C: Calc> Storage for DynamicStorage<C>
where
    CalcHolder<C, DynamicCapacity>: IndexCalculation,
{
    type Calc = CalcHolder<C, DynamicCapacity>;

    fn calc(&self) -> &Self::Calc {
        &self.calc
    }
}

pub struct ArrayStorage<const N: usize, C: Calc> {
    calc: CalcHolder<C, ConstCapacity<N>>,
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
            calc: CalcHolder {
                cap: ConstCapacity,
                _phantom: PhantomData,
            },
        }
    }
}

impl<const N: usize, C: Calc> Storage for ArrayStorage<N, C>
where
    CalcHolder<C, ConstCapacity<N>>: IndexCalculation,
{
    type Calc = CalcHolder<C, ConstCapacity<N>>;

    fn calc(&self) -> &Self::Calc {
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
    let p = Producer::new(DynamicStorage::<DoubleLength>::new(14));
    let n = 27;
    println!("increment {} (capacity {}) -> {}", n, p.capacity(), p.push(n));

    let p = Producer::new(ArrayStorage::<16, DoubleLengthPowerOfTwo>::new());
    println!("increment {} (capacity {}) -> {}", n, p.capacity(), p.push(n));
}
