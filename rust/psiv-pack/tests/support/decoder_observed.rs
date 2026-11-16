//! Per-thread, test-only decoder-call census, including refused calls.
use super::Call;
use std::{cell::RefCell, collections::HashSet};

thread_local! {
    static CALLS: RefCell<Option<HashSet<Call>>> = const { RefCell::new(None) };
}

pub(super) fn record(call: Call) {
    CALLS.with_borrow_mut(|calls| {
        if let Some(calls) = calls {
            calls.insert(call);
        }
    });
}

pub(crate) fn during<T>(run: impl FnOnce() -> T) -> (T, Vec<Call>) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            CALLS.with_borrow_mut(|calls| *calls = None);
        }
    }
    CALLS.with_borrow_mut(|calls| {
        assert!(calls.is_none(), "nested decoder observation");
        *calls = Some(HashSet::new());
    });
    let reset = Reset;
    let result = run();
    let calls = CALLS.with_borrow_mut(|calls| calls.take().unwrap().into_iter().collect());
    drop(reset);
    (result, calls)
}

#[test]
fn observes_direct_and_cached_calls_including_errors_and_resets_after_panic() {
    use super::{Decodes, enigma, kosinski, nemesis};
    let (_, calls) = during(|| {
        assert!(nemesis::decompress(&[], 7).is_err());
        assert!(kosinski::decompress(&[], 9).is_err());
        assert!(enigma::decompress(&[], 11, 3, Some(8)).is_err());
        let cache = Decodes::new(&[]);
        assert!(cache.get(Call::Nemesis { offset: 7 }).is_err());
    });
    assert_eq!(
        calls.into_iter().collect::<HashSet<_>>(),
        HashSet::from([
            Call::Nemesis { offset: 7 },
            Call::Kosinski { offset: 9 },
            Call::Enigma {
                offset: 11,
                base_tile: 3,
                max_words: Some(8)
            },
        ])
    );
    assert!(std::panic::catch_unwind(|| during(|| panic!("test scope reset"))).is_err());
    assert!(during(|| ()).1.is_empty());
}
