//! Run with `cargo kani -p console-kernel --lib --harness interrupts::proofs`.

use super::{
  Address, Arrival, Bytes, Exception, Gate, InterruptAction, Segments, TableError, TaskState,
  Vector, stack_top,
};

#[kani::proof]
fn only_the_syscall_gate_accepts_user_software_interrupts() {
  let first: u64 = kani::any();
  let number: u64 = kani::any();
  let address = u128::from(first) + 16 * u128::from(number);

  match Gate::of(Address(first), Vector(number)) {
    Ok(gate) => {
      let [
        one,
        two,
        selector_low,
        selector_high,
        stack,
        flags,
        three,
        four,
      ] = gate.low.to_le_bytes();
      let [
        five,
        six,
        seven,
        eight,
        zero,
        one_reserved,
        two_reserved,
        three_reserved,
      ] = gate.high.to_le_bytes();
      let handler = u64::from_le_bytes([one, two, three, four, five, six, seven, eight]);

      assert!(number < 256);
      assert_eq!(u128::from(handler), address);
      assert_eq!(u16::from_le_bytes([selector_low, selector_high]), 8);
      assert_eq!(stack, if number == 8 { 1 } else { 0 });
      assert_eq!(flags & !0x60, 0x8e);
      assert_eq!(flags & 0x60, if number == 128 { 0x60 } else { 0 });
      assert_eq!([zero, one_reserved, two_reserved, three_reserved], [0; 4]);
    }
    Err(TableError::InvalidVector(Vector(refused))) => {
      assert_eq!(number, refused);
      assert!(number >= 256);
    }
    Err(TableError::Overflow) => {
      assert!(number < 256);
      assert!(address > u128::from(u64::MAX));
    }
    Err(_) => panic!("unexpected gate error"),
  }
}

#[kani::proof]
fn user_traps_and_double_faults_use_their_assigned_kernel_stacks() {
  let normal: u64 = kani::any();
  let emergency: u64 = kani::any();
  let descriptor: u64 = kani::any();
  let Ok(task) = TaskState::new(Address(normal), Address(emergency));
  let Ok(segments) = Segments::new(Address(descriptor));
  let [rsp0, rsp1, rsp2] = task.privilege_stacks;
  let [ist1, ist2, ist3, ist4, ist5, ist6, ist7] = task.interrupt_stacks;
  let [task_low, task_high] = segments.task_state;
  let [
    limit_low,
    limit_mid,
    first,
    second,
    third,
    access,
    limit_high,
    fourth,
  ] = task_low.to_le_bytes();
  let [fifth, sixth, seventh, eighth, ..] = task_high.to_le_bytes();
  let limit = u32::from_le_bytes([limit_low, limit_mid, limit_high & 0xf, 0]);

  assert_eq!(u64::from(rsp0.low) | (u64::from(rsp0.high) << 32), normal);
  assert_eq!(
    u64::from(ist1.low) | (u64::from(ist1.high) << 32),
    emergency
  );
  assert_eq!(
    [rsp1, rsp2, ist2, ist3, ist4, ist5, ist6, ist7].map(|stack| stack.low | stack.high),
    [0; 8]
  );
  assert_eq!(
    u64::from_le_bytes([first, second, third, fourth, fifth, sixth, seventh, eighth]),
    descriptor
  );
  assert_eq!(access, 0x89);
  assert_eq!(limit_high & 0x80, 0);
  assert_eq!(limit, 103);
  assert_eq!(u32::from(task.io_map_base), 104);
}

#[kani::proof]
fn a_kernel_stack_top_is_aligned_and_inside_its_reserved_range() {
  let start: u64 = kani::any();
  let length: u64 = kani::any();
  let end = u128::from(start) + u128::from(length);
  let rounded_down = end & !15;

  match stack_top(Address(start), Bytes(length)) {
    Ok(Address(top)) => {
      assert!(end <= u128::from(u64::MAX));
      assert!(top > start);
      assert_eq!(u128::from(top), rounded_down);
    }
    Err(TableError::Empty) => {
      assert!(end <= u128::from(u64::MAX));
      assert!(rounded_down <= u128::from(start));
    }
    Err(TableError::Overflow) => assert!(end > u128::from(u64::MAX)),
    Err(_) => panic!("unexpected stack error"),
  }
}

const EXCEPTIONS: [Arrival; 32] = [
  Arrival::Exception(Exception::DivideError),
  Arrival::Exception(Exception::Debug),
  Arrival::Exception(Exception::NonMaskableInterrupt),
  Arrival::Breakpoint,
  Arrival::Exception(Exception::Overflow),
  Arrival::Exception(Exception::BoundRangeExceeded),
  Arrival::Exception(Exception::InvalidOpcode),
  Arrival::Exception(Exception::DeviceNotAvailable),
  Arrival::Exception(Exception::DoubleFault),
  Arrival::Exception(Exception::Reserved),
  Arrival::Exception(Exception::InvalidTaskState),
  Arrival::Exception(Exception::SegmentNotPresent),
  Arrival::Exception(Exception::StackSegmentFault),
  Arrival::Exception(Exception::GeneralProtection),
  Arrival::Exception(Exception::PageFault),
  Arrival::Exception(Exception::Reserved),
  Arrival::Exception(Exception::FloatingPoint),
  Arrival::Exception(Exception::AlignmentCheck),
  Arrival::Exception(Exception::MachineCheck),
  Arrival::Exception(Exception::SimdFloatingPoint),
  Arrival::Exception(Exception::Virtualization),
  Arrival::Exception(Exception::ControlProtection),
  Arrival::Exception(Exception::Reserved),
  Arrival::Exception(Exception::Reserved),
  Arrival::Exception(Exception::Reserved),
  Arrival::Exception(Exception::Reserved),
  Arrival::Exception(Exception::Reserved),
  Arrival::Exception(Exception::Reserved),
  Arrival::Exception(Exception::Reserved),
  Arrival::Exception(Exception::Reserved),
  Arrival::Exception(Exception::Reserved),
  Arrival::Exception(Exception::Reserved),
];

#[kani::proof]
fn the_generic_dispatcher_decodes_every_vector_into_its_declared_event() {
  let number: u64 = kani::any();
  let Ok(actual) = Arrival::of(Vector(number));
  let expected = match number {
    0..32 => *EXCEPTIONS.get(usize::try_from(number).unwrap()).unwrap(),
    32 => Arrival::Tick,
    255 => Arrival::Spurious,
    _ => Arrival::Unexpected,
  };

  assert_eq!(actual, expected);
}

#[kani::proof]
fn only_breakpoints_timer_ticks_and_spurious_interrupts_resume_generic_dispatch() {
  let number: u64 = kani::any();
  let Ok(arrival) = Arrival::of(Vector(number));
  let Ok(action) = arrival.action();
  let expected = match number {
    3 => InterruptAction::CountBreakpoint,
    32 => InterruptAction::CountTick,
    255 => InterruptAction::Resume,
    _ => InterruptAction::Stop,
  };

  assert_eq!(action, expected);
}
