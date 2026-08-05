//! Invariants that must hold for *any* input, not just the recorded corpus.
//!
//! `tests/oracle.rs` proves the Rust kernel reproduces the JavaScript oracle on
//! a fixed set of cases. This file proves the properties those cases were
//! sampling: it is the Rust home of the assertions that used to live only in
//! `test/ttid.test.js`, restated over a range of timestamps rather than one.
//!
//! The distinction is what allowed the JavaScript engine to be retired: the
//! corpus outlives it as a frozen fixture, but these invariants would have gone
//! with it. See `docs/TEST_MIGRATION.md`.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use ttid::ttid::{self as kernel, Generator};

const MIN_MS: f64 = 1_577_836_800_000.0; // 2020-01-01
const MAX_MS: f64 = 7_258_118_400_000.0; // 2200-01-01

/// A spread of timestamps across the supported range, so an invariant is
/// checked against many encodings rather than whichever one the clock produced.
///
/// The top is held a few milliseconds below `MAX_MS`: [`lifecycle`] stamps at
/// `now + 1` and `now + 2`, and a segment past 2200 is out of range by
/// definition. The exact bounds get their own test rather than being smuggled
/// into every other one.
fn timestamps() -> Vec<f64> {
    let top = MAX_MS - 10.0;
    let mut values = vec![MIN_MS, top, 1_754_179_200_000.0];
    for step in 0..200 {
        values.push(MIN_MS + (top - MIN_MS) * f64::from(step) / 200.0);
    }
    for offset in [0.1_f64, 0.25, 0.5, 0.7531, 0.9999] {
        values.push(1_754_179_200_000.0 + offset);
    }
    values
}

#[test]
fn the_exact_bounds_encode_and_decode() {
    for now in [MIN_MS, MAX_MS] {
        let id = kernel::generate(None, false, now).unwrap();
        assert_eq!(id.len(), 11);
        #[expect(clippy::cast_possible_truncation, reason = "both bounds are integral")]
        let expected = now as i64;
        assert_eq!(kernel::decode_time(&id).unwrap().created_at, expected);
        assert_eq!(kernel::is_ttid(&id), Some(expected));
    }
}

#[test]
fn a_stamp_outside_the_bounds_is_rejected_on_decode() {
    // `generate` does not range-check — that is the oracle's behavior — so an
    // id minted outside 2020..2200 is well-formed but undecodable.
    for now in [MIN_MS - 1000.0, MAX_MS + 1000.0] {
        let id = kernel::generate(None, false, now).unwrap();
        assert_eq!(id.len(), 11, "still well-formed: {id}");
        assert_eq!(
            kernel::decode_time(&id).unwrap_err().0,
            "Invalid timestamp encoding",
            "{id} at {now} should not decode"
        );
        assert_eq!(kernel::is_ttid(&id), None);
    }
}

/// The lifecycle, built at three increasing instants.
fn lifecycle(now: f64) -> (String, String, String) {
    let created = kernel::generate(None, false, now).unwrap();
    let updated = kernel::generate(Some(&created), false, now + 1.0).unwrap();
    let deleted = kernel::generate(Some(&updated), true, now + 2.0).unwrap();
    (created, updated, deleted)
}

// --- output format --------------------------------------------------------

#[test]
fn generated_ids_are_always_uppercase() {
    for now in timestamps() {
        let id = kernel::generate(None, false, now).unwrap();
        assert_eq!(id, id.to_uppercase(), "not uppercase at {now}");
    }
}

#[test]
fn a_single_segment_id_is_exactly_eleven_characters() {
    for now in timestamps() {
        let id = kernel::generate(None, false, now).unwrap();
        assert_eq!(id.len(), 11, "{id} at {now} is not 11 characters");
    }
}

#[test]
fn a_two_segment_id_has_two_eleven_character_segments() {
    for now in timestamps() {
        let created = kernel::generate(None, false, now).unwrap();
        let updated = kernel::generate(Some(&created), false, now + 1.0).unwrap();
        let segments: Vec<&str> = updated.split('-').collect();
        assert_eq!(segments.len(), 2, "{updated}");
        assert!(
            segments.iter().all(|segment| segment.len() == 11),
            "{updated}"
        );
    }
}

#[test]
fn a_placeholder_id_has_eleven_one_eleven_segments() {
    for now in timestamps() {
        let created = kernel::generate(None, false, now).unwrap();
        let deleted = kernel::generate(Some(&created), true, now + 1.0).unwrap();
        let segments: Vec<&str> = deleted.split('-').collect();
        assert_eq!(segments.len(), 3, "{deleted}");
        assert_eq!(segments[0].len(), 11);
        assert_eq!(
            segments[1], "X",
            "deleting without an update must use the placeholder"
        );
        assert_eq!(segments[2].len(), 11);
    }
}

#[test]
fn updating_then_deleting_leaves_no_placeholder() {
    for now in timestamps() {
        let (_, _, deleted) = lifecycle(now);
        let segments: Vec<&str> = deleted.split('-').collect();
        assert_eq!(segments.len(), 3, "{deleted}");
        assert_ne!(segments[1], "X", "an updated id must keep its update stamp");
        assert!(
            segments.iter().all(|segment| segment.len() == 11),
            "{deleted}"
        );
    }
}

// --- lifecycle ------------------------------------------------------------

#[test]
fn lifecycle_timestamps_are_chronological() {
    for now in timestamps() {
        let (_, _, deleted) = lifecycle(now);
        let times = kernel::decode_time(&deleted).unwrap();
        let updated_at = times.updated_at.expect("an updated id decodes updatedAt");
        let deleted_at = times.deleted_at.expect("a deleted id decodes deletedAt");
        assert!(
            times.created_at <= updated_at,
            "{deleted}: created > updated"
        );
        assert!(updated_at <= deleted_at, "{deleted}: updated > deleted");
    }
}

#[test]
fn decode_time_arity_matches_segment_count() {
    for now in timestamps() {
        let (created, updated, deleted) = lifecycle(now);

        let one = kernel::decode_time(&created).unwrap();
        assert!(one.updated_at.is_none() && one.deleted_at.is_none());

        let two = kernel::decode_time(&updated).unwrap();
        assert!(two.updated_at.is_some() && two.deleted_at.is_none());

        let three = kernel::decode_time(&deleted).unwrap();
        assert!(three.updated_at.is_some() && three.deleted_at.is_some());
    }
}

#[test]
fn a_placeholder_decodes_no_update_but_a_deletion() {
    for now in timestamps() {
        let created = kernel::generate(None, false, now).unwrap();
        let deleted = kernel::generate(Some(&created), true, now + 1.0).unwrap();
        let times = kernel::decode_time(&deleted).unwrap();
        assert!(
            times.updated_at.is_none(),
            "the X placeholder must not decode"
        );
        assert!(times.deleted_at.is_some());
    }
}

#[test]
fn is_ttid_always_agrees_with_decode_time() {
    for now in timestamps() {
        let (created, updated, deleted) = lifecycle(now);
        for id in [&created, &updated, &deleted] {
            let from_is_ttid = kernel::is_ttid(id).expect("a generated id is valid");
            let from_decode = kernel::decode_time(id).unwrap().created_at;
            assert_eq!(
                from_is_ttid, from_decode,
                "{id}: isTTID must report the creation time, not the last segment"
            );
        }
    }
}

#[test]
fn every_segment_round_trips_to_the_instant_that_made_it() {
    for now in timestamps() {
        let id = kernel::generate(None, false, now).unwrap();
        let decoded = kernel::decode_time(&id).unwrap().created_at;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the range is bounded by 2200"
        )]
        let expected = now.round() as i64;
        assert_eq!(
            decoded, expected,
            "{id} decoded to {decoded}, expected {expected}"
        );
    }
}

// --- rejection ------------------------------------------------------------

#[test]
fn generate_rejects_a_deleted_id_either_way() {
    for now in timestamps() {
        let (_, _, deleted) = lifecycle(now);
        for delete in [false, true] {
            let error = kernel::generate(Some(&deleted), delete, now + 3.0).unwrap_err();
            assert_eq!(error.0, "This identifier can no longer be modified");
        }
    }
}

#[test]
fn generate_rejects_non_ttid_input() {
    for input in [
        "not-a-valid-ttid",
        "3f2504e0-4f89-41d3-9a0c-0305e82c3301", // a UUID
        "0192f7a1-9c2e-7b3d-8f4a-1c2d3e4f5a6b", // a v7 UUID
        "ABCDEFGHIJ",                           // ten characters
        "ABCDEFGHIJKL",                         // twelve
        "ABCDEFGHIJ!",                          // non-alphanumeric
        "00000000000",                          // in format, out of range
    ] {
        let error = kernel::generate(Some(input), false, 1_754_179_200_000.0).unwrap_err();
        assert_eq!(error.0, "Invalid TTID!", "input: {input}");
    }
}

#[test]
fn delete_false_behaves_exactly_like_omitting_the_flag() {
    for now in timestamps() {
        let created = kernel::generate(None, false, now).unwrap();
        // Both spellings of "not a delete" must produce the same two-segment id.
        let implicit = kernel::generate(Some(&created), false, now + 1.0).unwrap();
        assert_eq!(implicit.split('-').count(), 2, "{implicit}");
        assert!(implicit.starts_with(&created));

        // And a fresh id with the flag set explicitly false is still one segment.
        let fresh = kernel::generate(None, false, now).unwrap();
        assert_eq!(
            fresh, created,
            "an explicit false must not change the encoding"
        );
    }
}

#[test]
fn an_empty_id_creates_a_new_one_rather_than_failing() {
    let id = kernel::generate(Some(""), false, 1_754_179_200_000.0).unwrap();
    assert_eq!(id.len(), 11);
}

#[test]
fn is_ttid_rejects_what_it_should() {
    for input in [
        "",
        "3f2504e0-4f89-41d3-9a0c-0305e82c3301",
        "0192f7a1-9c2e-7b3d-8f4a-1c2d3e4f5a6b",
        "00000000000", // valid format, out-of-range timestamp
        "not-a-ttid",
        "-",
    ] {
        assert_eq!(kernel::is_ttid(input), None, "input: {input}");
    }
    // The length short-circuit runs before the pattern test.
    assert_eq!(kernel::is_ttid(&"A".repeat(37)), None);
    assert_eq!(kernel::is_ttid(&"A".repeat(36)), None);
}

#[test]
fn nothing_longer_than_a_full_lifecycle_id_can_be_valid() {
    // 11 + 1 + 11 + 1 + 11 = 35 characters is the longest a TTID can be.
    for length in 36..=200 {
        assert_eq!(
            kernel::is_ttid(&"A".repeat(length)),
            None,
            "length {length}"
        );
        assert_eq!(
            kernel::is_ttid(&"4".repeat(length)),
            None,
            "length {length}"
        );
    }

    // Note for anyone mutation-testing this file: the `> 36` guard inside
    // `is_ttid` is a cost control, not a boundary. The pattern already rejects
    // everything past 35, so widening the guard changes how long a rejection
    // takes, never its answer — no assertion here can catch that, and none
    // should pretend to.
}

#[test]
fn decode_time_rejects_what_it_should() {
    for input in [
        "",
        "-",
        "not-a-ttid",
        "ABCDEFGHIJ",
        "ABCDEFGHIJKL",
        "ABCDEFGHIJ!",
    ] {
        assert_eq!(
            kernel::decode_time(input).unwrap_err().0,
            "Invalid Format!",
            "input: {input}"
        );
    }
    // Well-formed but outside 2020..2200.
    assert_eq!(
        kernel::decode_time("00000000000").unwrap_err().0,
        "Invalid timestamp encoding"
    );
}

// --- case handling --------------------------------------------------------

#[test]
fn lowercase_input_is_accepted_and_normalized() {
    for now in timestamps() {
        let id = kernel::generate(None, false, now).unwrap();
        let lowered = id.to_lowercase();

        assert_eq!(kernel::is_ttid(&lowered), kernel::is_ttid(&id));
        assert_eq!(kernel::decode_time(&lowered), kernel::decode_time(&id));

        let updated = kernel::generate(Some(&lowered), false, now + 1.0).unwrap();
        assert_eq!(updated, updated.to_uppercase(), "output must be uppercased");
        assert!(updated.starts_with(&id));
    }
}

// --- uuid -----------------------------------------------------------------

#[test]
fn is_uuid_accepts_every_version() {
    for uuid in [
        "3f2504e0-4f89-41d3-9a0c-0305e82c3301", // v4
        "0192f7a1-9c2e-7b3d-8f4a-1c2d3e4f5a6b", // v7
        "00000000-0000-0000-0000-000000000000", // nil
        "ffffffff-ffff-ffff-ffff-ffffffffffff", // max
        "3F2504E0-4F89-41D3-9A0C-0305E82C3301", // uppercase
    ] {
        assert!(kernel::is_uuid(uuid), "should be a uuid: {uuid}");
    }
}

#[test]
fn is_uuid_rejects_what_it_should() {
    for input in [
        "",
        "not-a-uuid",
        "3f2504e0-4f89-41d3-9a0c-0305e82c330",   // one short
        "3f2504e0-4f89-41d3-9a0c-0305e82c33011", // one long
        "3f2504e0-4f89-41d3-9a0c-0305e82c330g",  // non-hex
        "3f2504e04f8941d39a0c0305e82c3301",      // no dashes
    ] {
        assert!(!kernel::is_uuid(input), "should not be a uuid: {input}");
    }
    // A TTID is never a UUID.
    for now in timestamps() {
        let (created, updated, deleted) = lifecycle(now);
        for id in [&created, &updated, &deleted] {
            assert!(!kernel::is_uuid(id), "a ttid is not a uuid: {id}");
        }
    }
}

// --- the monotonic generator preserves every invariant above --------------

#[test]
fn the_generator_holds_the_format_invariants_under_a_frozen_clock() {
    let mut generator = Generator::new();
    let frozen = 1_754_179_200_000.0;

    for _ in 0..5_000 {
        let id = generator.generate(None, false, frozen).unwrap();
        assert_eq!(id.len(), 11);
        assert_eq!(id, id.to_uppercase());
        assert!(kernel::is_ttid(&id).is_some(), "{id} must stay valid");

        let updated = generator.generate(Some(&id), false, frozen).unwrap();
        assert_eq!(updated.split('-').count(), 2);
        assert!(updated.starts_with(&id));

        let deleted = generator.generate(Some(&updated), true, frozen).unwrap();
        assert_eq!(deleted.split('-').count(), 3);
        let times = kernel::decode_time(&deleted).unwrap();
        assert!(times.created_at <= times.updated_at.unwrap());
        assert!(times.updated_at.unwrap() <= times.deleted_at.unwrap());
    }
}

// --- canonical form -------------------------------------------------------
// Issue #32: identifiers are matched case-insensitively but only ever emitted
// in uppercase, so string equality is not identity. `canonical` is what a
// consumer normalizes with before storing or comparing.

#[test]
fn canonical_is_uppercase_for_every_accepted_spelling() {
    for now in timestamps() {
        let (created, updated, deleted) = lifecycle(now);
        for id in [&created, &updated, &deleted] {
            for spelling in [id.clone(), id.to_lowercase(), mixed_case(id)] {
                let canonical = kernel::canonical(&spelling)
                    .unwrap_or_else(|| panic!("{spelling} should be canonicalizable"));
                assert_eq!(canonical, *id, "every spelling must collapse to one form");
                assert_eq!(canonical, canonical.to_uppercase());
            }
        }
    }
}

#[test]
fn canonical_is_idempotent() {
    for now in timestamps() {
        let id = kernel::generate(None, false, now).unwrap();
        let once = kernel::canonical(&id.to_lowercase()).unwrap();
        let twice = kernel::canonical(&once).unwrap();
        assert_eq!(once, twice);
    }
}

#[test]
fn canonical_preserves_the_decoded_instant() {
    for now in timestamps() {
        let (_, _, deleted) = lifecycle(now);
        let canonical = kernel::canonical(&deleted.to_lowercase()).unwrap();
        assert_eq!(
            kernel::decode_time(&canonical).unwrap(),
            kernel::decode_time(&deleted).unwrap(),
            "normalizing must not change what an id means"
        );
    }
}

#[test]
fn canonical_rejects_what_is_not_a_ttid() {
    for input in [
        "",
        "not-a-ttid",
        "3f2504e0-4f89-41d3-9a0c-0305e82c3301",
        "00000000000", // in format, out of range
        "ABCDEFGHIJ",
    ] {
        assert_eq!(kernel::canonical(input), None, "input: {input}");
    }
}

/// Canonical ids sort chronologically by byte comparison; non-canonical ones do
/// not, which is the sorting half of issue #32.
#[test]
fn canonicalizing_restores_chronological_sort_order() {
    let mut generator = Generator::new();
    let base = 1_754_179_200_000.0;
    let ids: Vec<String> = (0..500)
        .map(|step| {
            generator
                .generate(None, false, base + f64::from(step))
                .unwrap()
        })
        .collect();

    // Spell every other one in lowercase, as a careless consumer might store it.
    let mixed: Vec<String> = ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            if index % 2 == 0 {
                id.to_lowercase()
            } else {
                id.clone()
            }
        })
        .collect();

    let mut sorted_mixed = mixed.clone();
    sorted_mixed.sort();
    assert_ne!(
        sorted_mixed, mixed,
        "mixed case must break byte-order sorting"
    );

    let mut normalized: Vec<String> = mixed
        .iter()
        .filter_map(|id| kernel::canonical(id))
        .collect();
    assert_eq!(normalized.len(), ids.len());
    let before = normalized.clone();
    normalized.sort();
    assert_eq!(normalized, before, "canonical ids sort chronologically");
    assert_eq!(normalized, ids);
}

/// Alternate the case of the letters, leaving digits alone.
fn mixed_case(id: &str) -> String {
    id.chars()
        .enumerate()
        .map(|(index, character)| {
            if index % 2 == 0 {
                character.to_ascii_lowercase()
            } else {
                character.to_ascii_uppercase()
            }
        })
        .collect()
}
