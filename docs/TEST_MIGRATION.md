# Test Migration — JavaScript to Rust

The record of what happened to every test in the retired JavaScript suite, so
the deletion of `legacy/` can be audited rather than trusted.

Source suites, all removed:

- `test/ttid.test.js` — 32 tests
- `test/cli.test.js` — 6 tests
- `test/uniqueness.test.js` — 6 tests × 2 targets (library, web client)

Nothing was dropped. Several JavaScript tests collapse into one Rust test
because the Rust version asserts the same property over ~208 timestamps
spanning 2020–2200, where the JavaScript one used a single reading of the
clock.

## `test/ttid.test.js`

| JavaScript test | Rust home |
| --- | --- |
| Generate | `invariants::a_single_segment_id_is_exactly_eleven_characters` |
| Update | `invariants::a_two_segment_id_has_two_eleven_character_segments` |
| Created-Deleted | `invariants::a_placeholder_id_has_eleven_one_eleven_segments` |
| Created-Updated-Deleted | `invariants::updating_then_deleting_leaves_no_placeholder` |
| 2-segment + del:true produces 3-segment without placeholder | `invariants::updating_then_deleting_leaves_no_placeholder` |
| del:false behaves the same as omitting the flag | `invariants::delete_false_behaves_exactly_like_omitting_the_flag` |
| throws when given a UUID string | `invariants::generate_rejects_non_ttid_input` (v4 and v7) |
| throws when modifying a deleted TTID | `invariants::generate_rejects_a_deleted_id_either_way` |
| throws when given an invalid TTID string | `invariants::generate_rejects_non_ttid_input` |
| successive calls produce unique IDs | `oracle::a_burst_on_a_frozen_clock_never_repeats` |
| isTTID: empty string returns null | `invariants::is_ttid_rejects_what_it_should` |
| isTTID: 1-segment returns creation Date | `invariants::is_ttid_always_agrees_with_decode_time` |
| isTTID: 2-segment returns creation, not update | `invariants::is_ttid_always_agrees_with_decode_time` |
| isTTID: 3-segment returns creation, not deletion | `invariants::is_ttid_always_agrees_with_decode_time` |
| isTTID: valid format, out-of-range returns null | `invariants::is_ttid_rejects_what_it_should` |
| isTTID: UUID returns null | `invariants::is_ttid_rejects_what_it_should` |
| isTTID: rejects oversized input | `invariants::nothing_longer_than_a_full_lifecycle_id_can_be_valid` |
| decodeTime: 1-segment has only createdAt | `invariants::decode_time_arity_matches_segment_count` |
| decodeTime: 2-segment has updatedAt, no deletedAt | `invariants::decode_time_arity_matches_segment_count` |
| decodeTime: placeholder has no updatedAt, has deletedAt | `invariants::a_placeholder_decodes_no_update_but_a_deletion` |
| decodeTime: chronological across the lifecycle | `invariants::lifecycle_timestamps_are_chronological` |
| decodeTime: throws on invalid format | `invariants::decode_time_rejects_what_it_should` |
| decodeTime: throws on out-of-range segment | `invariants::decode_time_rejects_what_it_should` |
| generated IDs are always uppercase | `invariants::generated_ids_are_always_uppercase` |
| single-segment ID is exactly 11 characters | `invariants::a_single_segment_id_is_exactly_eleven_characters` |
| two-segment ID has two 11-character segments | `invariants::a_two_segment_id_has_two_eleven_character_segments` |
| three-segment ID has correct segment lengths | `invariants::a_placeholder_id_has_eleven_one_eleven_segments` |
| isTTID getTime consistent with decodeTime createdAt | `invariants::is_ttid_always_agrees_with_decode_time` |
| isUUID: TTID returns null | `invariants::is_uuid_rejects_what_it_should` |
| isUUID: valid v4 returns a match | `invariants::is_uuid_accepts_every_version` |
| isUUID: valid v7 returns a match | `invariants::is_uuid_accepts_every_version` |
| isUUID: empty string returns null | `invariants::is_uuid_rejects_what_it_should` |

## `test/cli.test.js`

| JavaScript test | Rust home |
| --- | --- |
| generates a TTID through the machine interface | `machine::generates_a_ttid_through_the_machine_interface` |
| updates and deletes through arguments | `machine::updates_and_deletes_an_existing_ttid_through_arguments` |
| decodes timestamps | `machine::decodes_timestamps_through_the_machine_interface` |
| validates TTIDs | `machine::validates_ttids_through_the_machine_interface` |
| returns structured errors | `machine::returns_structured_errors` |
| rejects unsupported operations | `machine::rejects_unsupported_operations_through_the_machine_interface` |

## `test/uniqueness.test.js`

| JavaScript test | Rust home |
| --- | --- |
| a tight loop produces no duplicates | `oracle::a_burst_on_a_frozen_clock_never_repeats` |
| ids stay strictly increasing | `oracle::a_burst_stays_strictly_increasing_and_sortable` |
| ids stay 11 characters | `invariants::the_generator_holds_the_format_invariants_under_a_frozen_clock` |
| a burst barely moves the decoded timestamp | `oracle::a_burst_barely_moves_the_decoded_timestamp` |
| every id in a burst is still valid | `invariants::the_generator_holds_the_format_invariants_under_a_frozen_clock` |
| the lifecycle still holds under a burst | `invariants::the_generator_holds_the_format_invariants_under_a_frozen_clock` |

The web-client half of this suite is **not** ported to Rust — it tests
`clients/web/ttid.mjs`, which is shipped JavaScript, not the retired engine. It
is covered instead by `scripts/wasm/browser-test.mjs`, which runs the same
uniqueness checks against that file in a real browser on every CI run.

## Coverage added that JavaScript never had

Beyond the port, the Rust suite asserts things the old one did not:

- Byte-for-byte replay of 79 recorded oracle cases (`oracle.rs`).
- The exact 2020 and 2200 bounds, and rejection outside them.
- Every generated id decoding back to the instant that produced it.
- Lowercase input normalising identically across all four operations.
- Malformed protocol envelopes, and `id` field validation across all three
  ops that read one (`machine.rs`).
- The indented CLI output matching `JSON.stringify(value, null, 2)` exactly.
- A frozen-clock burst holding every format invariant simultaneously.

## How this was checked

Mutation testing, not inspection. Five deliberate breaks to `src/ttid.rs` —
lowercase digits, a changed placeholder, `is_ttid` returning the wrong segment,
a widened length guard, a UUID group dropped — were each run against the suite.
Four were caught by two to four tests apiece. The fifth (widening the `> 36`
guard) is not catchable and is documented as such in `invariants.rs`: the
longest valid TTID is 35 characters, so that guard controls cost, never answers.
