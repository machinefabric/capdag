# CapDag Test Catalog

**Total Tests:** 923

This catalog lists all numbered tests in the capdag codebase.

| Test # | Function Name | Description | Location |
|--------|---------------|-------------|----------|
| test001 | `test001_cap_urn_creation` | TEST001: Test that cap URN is created with tags parsed correctly and direction specs accessible | src/urn/cap_urn.rs:901 |
| test002 | `test002_direction_specs_default_to_wildcard` | TEST002: Test that missing 'in' or 'out' defaults to media: wildcard | src/urn/cap_urn.rs:913 |
| test003 | `test003_direction_matching` | TEST003: Test that direction specs must match exactly, different in/out types don't match, wildcard matches any | src/urn/cap_urn.rs:935 |
| test004 | `test004_unquoted_values_lowercased` | TEST004: Test that unquoted keys and values are normalized to lowercase | src/urn/cap_urn.rs:964 |
| test005 | `test005_quoted_values_preserve_case` | TEST005: Test that quoted values preserve case while unquoted are lowercased | src/urn/cap_urn.rs:985 |
| test006 | `test006_quoted_value_special_chars` | TEST006: Test that quoted values can contain special characters (semicolons, equals, spaces) | src/urn/cap_urn.rs:1004 |
| test007 | `test007_quoted_value_escape_sequences` | TEST007: Test that escape sequences in quoted values (\" and \\) are parsed correctly | src/urn/cap_urn.rs:1023 |
| test008 | `test008_mixed_quoted_unquoted` | TEST008: Test that mixed quoted and unquoted values in same URN parse correctly | src/urn/cap_urn.rs:1042 |
| test009 | `test009_unterminated_quote_error` | TEST009: Test that unterminated quote produces UnterminatedQuote error | src/urn/cap_urn.rs:1050 |
| test010 | `test010_invalid_escape_sequence_error` | TEST010: Test that invalid escape sequences (like \n, \x) produce InvalidEscapeSequence error | src/urn/cap_urn.rs:1060 |
| test011 | `test011_serialization_smart_quoting` | TEST011: Test that serialization uses smart quoting (no quotes for simple lowercase, quotes for special chars/uppercase) | src/urn/cap_urn.rs:1077 |
| test012 | `test012_round_trip_simple` | TEST012: Test that simple cap URN round-trips (parse -> serialize -> parse equals original) | src/urn/cap_urn.rs:1112 |
| test013 | `test013_round_trip_quoted` | TEST013: Test that quoted values round-trip preserving case and spaces | src/urn/cap_urn.rs:1122 |
| test014 | `test014_round_trip_escapes` | TEST014: Test that escape sequences round-trip correctly | src/urn/cap_urn.rs:1136 |
| test015 | `test015_cap_prefix_required` | TEST015: Test that cap: prefix is required and case-insensitive | src/urn/cap_urn.rs:1150 |
| test016 | `test016_trailing_semicolon_equivalence` | TEST016: Test that trailing semicolon is equivalent (same hash, same string, matches) | src/urn/cap_urn.rs:1173 |
| test017 | `test017_tag_matching` | TEST017: Test tag matching: exact match, subset match, wildcard match, value mismatch | src/urn/cap_urn.rs:1206 |
| test018 | `test018_matching_case_sensitive_values` | TEST018: Test that quoted values with different case do NOT match (case-sensitive) | src/urn/cap_urn.rs:1233 |
| test019 | `test019_missing_tag_handling` | TEST019: Missing tag in instance causes rejection — pattern's tags are constraints | src/urn/cap_urn.rs:1247 |
| test020 | `test020_specificity` | TEST020: Test specificity calculation (direction specs use MediaUrn tag count, wildcards don't count) | src/urn/cap_urn.rs:1266 |
| test021 | `test021_builder` | TEST021: Test builder creates cap URN with correct tags and direction specs | src/urn/cap_urn.rs:1286 |
| test022 | `test022_builder_requires_direction` | TEST022: Test builder requires both in_spec and out_spec | src/urn/cap_urn.rs:1303 |
| test023 | `test023_builder_preserves_case` | TEST023: Test builder lowercases keys but preserves value case | src/urn/cap_urn.rs:1328 |
| test024 | `test024_directional_accepts` | TEST024: Directional accepts — pattern's tags are constraints, instance must satisfy | src/urn/cap_urn.rs:1342 |
| test025 | `test025_best_match` | TEST025: Test find_best_match returns most specific matching cap | src/urn/cap_urn.rs:1373 |
| test026 | `test026_merge_and_subset` | TEST026: Test merge combines tags from both caps, subset keeps only specified tags | src/urn/cap_urn.rs:1389 |
| test027 | `test027_wildcard_tag` | TEST027: Test with_wildcard_tag sets tag to wildcard, including in/out | src/urn/cap_urn.rs:1413 |
| test028 | `test028_empty_cap_urn_defaults_to_wildcard` | TEST028: Test empty cap URN defaults to media: wildcard | src/urn/cap_urn.rs:1429 |
| test029 | `test029_minimal_cap_urn` | TEST029: Test minimal valid cap URN has just in and out, empty tags | src/urn/cap_urn.rs:1443 |
| test030 | `test030_extended_character_support` | TEST030: Test extended characters (forward slashes, colons) in tag values | src/urn/cap_urn.rs:1454 |
| test031 | `test031_wildcard_restrictions` | TEST031: Test wildcard rejected in keys but accepted in values | src/urn/cap_urn.rs:1467 |
| test032 | `test032_duplicate_key_rejection` | TEST032: Test duplicate keys are rejected with DuplicateKey error | src/urn/cap_urn.rs:1478 |
| test033 | `test033_numeric_key_restriction` | TEST033: Test pure numeric keys rejected, mixed alphanumeric allowed, numeric values allowed | src/urn/cap_urn.rs:1488 |
| test034 | `test034_empty_value_error` | TEST034: Test empty values are rejected | src/urn/cap_urn.rs:1502 |
| test035 | `test035_has_tag_case_sensitive` | TEST035: Test has_tag is case-sensitive for values, case-insensitive for keys, works for in/out | src/urn/cap_urn.rs:1509 |
| test036 | `test036_with_tag_preserves_value` | TEST036: Test with_tag preserves value case | src/urn/cap_urn.rs:1530 |
| test037 | `test037_with_tag_rejects_empty_value` | TEST037: Test with_tag rejects empty value | src/urn/cap_urn.rs:1539 |
| test038 | `test038_semantic_equivalence` | TEST038: Test semantic equivalence of unquoted and quoted simple lowercase values | src/urn/cap_urn.rs:1548 |
| test039 | `test039_get_tag_returns_direction_specs` | TEST039: Test get_tag returns direction specs (in/out) with case-insensitive lookup | src/urn/cap_urn.rs:1561 |
| test040 | `test040_matching_semantics_test1_exact_match` | TEST040: Matching semantics - exact match succeeds | src/urn/cap_urn.rs:1589 |
| test041 | `test041_matching_semantics_test2_cap_missing_tag` | TEST041: Matching semantics - cap missing tag matches (implicit wildcard) | src/urn/cap_urn.rs:1598 |
| test042 | `test042_matching_semantics_test3_cap_has_extra_tag` | TEST042: Pattern rejects instance missing required tags | src/urn/cap_urn.rs:1610 |
| test043 | `test043_matching_semantics_test4_request_has_wildcard` | TEST043: Matching semantics - request wildcard matches specific cap value | src/urn/cap_urn.rs:1621 |
| test044 | `test044_matching_semantics_test5_cap_has_wildcard` | TEST044: Matching semantics - cap wildcard matches specific request value | src/urn/cap_urn.rs:1633 |
| test045 | `test045_matching_semantics_test6_value_mismatch` | TEST045: Matching semantics - value mismatch does not match | src/urn/cap_urn.rs:1642 |
| test046 | `test046_matching_semantics_test7_fallback_pattern` | TEST046: Matching semantics - fallback pattern (cap missing tag = implicit wildcard) | src/urn/cap_urn.rs:1654 |
| test047 | `test047_matching_semantics_test7b_thumbnail_void_input` | TEST047: Matching semantics - thumbnail fallback with void input | src/urn/cap_urn.rs:1675 |
| test048 | `test048_matching_semantics_test8_wildcard_direction_matches_anything` | TEST048: Matching semantics - wildcard direction matches anything | src/urn/cap_urn.rs:1696 |
| test049 | `test049_matching_semantics_test9_cross_dimension_independence` | TEST049: Non-overlapping tags — neither direction accepts | src/urn/cap_urn.rs:1712 |
| test050 | `test050_matching_semantics_test10_direction_mismatch` | TEST050: Matching semantics - direction mismatch prevents matching | src/urn/cap_urn.rs:1722 |
| test051 | `test051_input_validation_success` | TEST051: Test input validation succeeds with valid positional argument | src/cap/validation.rs:1065 |
| test052 | `test052_input_validation_missing_required` | TEST052: Test input validation fails with MissingRequiredArgument when required arg missing | src/cap/validation.rs:1086 |
| test053 | `test053_input_validation_wrong_type` | TEST053: Test input validation fails with InvalidArgumentType when wrong type provided | src/cap/validation.rs:1114 |
| test054 | `test054_xv5_inline_spec_redefinition_detected` | TEST054: XV5 - Test inline media spec redefinition of existing registry spec is detected and rejected | src/cap/validation.rs:1156 |
| test055 | `test055_xv5_new_inline_spec_allowed` | TEST055: XV5 - Test new inline media spec (not in registry) is allowed | src/cap/validation.rs:1190 |
| test056 | `test056_xv5_empty_media_specs_allowed` | TEST056: XV5 - Test empty media_specs (no inline specs) passes XV5 validation | src/cap/validation.rs:1220 |
| test060 | `test060_wrong_prefix_fails` | TEST060: Test wrong prefix fails with InvalidPrefix error showing expected and actual prefix | src/urn/media_urn.rs:588 |
| test061 | `test061_is_binary` | TEST061: Test is_binary returns true when textable tag is absent (binary = not textable) | src/urn/media_urn.rs:601 |
| test062 | `test062_is_record` | TEST062: Test is_record returns true when record marker tag is present indicating key-value structure | src/urn/media_urn.rs:618 |
| test063 | `test063_is_scalar` | TEST063: Test is_scalar returns true when list marker tag is absent (scalar is default) | src/urn/media_urn.rs:631 |
| test064 | `test064_is_list` | TEST064: Test is_list returns true when list marker tag is present indicating ordered collection | src/urn/media_urn.rs:646 |
| test065 | `test065_is_opaque` | TEST065: Test is_opaque returns true when record marker is absent (opaque is default) | src/urn/media_urn.rs:659 |
| test066 | `test066_is_json` | TEST066: Test is_json returns true only when json marker tag is present for JSON representation | src/urn/media_urn.rs:673 |
| test067 | `test067_is_text` | TEST067: Test is_text returns true only when textable marker tag is present | src/urn/media_urn.rs:684 |
| test068 | `test068_is_void` | TEST068: Test is_void returns true when void flag or type=void tag is present | src/urn/media_urn.rs:697 |
| test071 | `test071_to_string_roundtrip` | TEST071: Test to_string roundtrip ensures serialization and deserialization preserve URN structure | src/urn/media_urn.rs:704 |
| test072 | `test072_constants_parse` | TEST072: Test all media URN constants parse successfully as valid media URNs | src/urn/media_urn.rs:714 |
| test073 | `test073_extension_helpers` | TEST073: Test extension helper functions create media URNs with ext tag and correct format | src/urn/media_urn.rs:748 |
| test074 | `test074_media_urn_matching` | TEST074: Test media URN conforms_to using tagged URN semantics with specific and generic requirements | src/urn/media_urn.rs:764 |
| test075 | `test075_matching` | TEST075: Test accepts with implicit wildcards where handlers with fewer tags can handle more requests | src/urn/media_urn.rs:784 |
| test076 | `test076_specificity` | TEST076: Test specificity increases with more tags for ranking conformance | src/urn/media_urn.rs:800 |
| test077 | `test077_serde_roundtrip` | TEST077: Test serde roundtrip serializes to JSON string and deserializes back correctly | src/urn/media_urn.rs:819 |
| test078 | `test078_object_does_not_conform_to_string` | TEST078: conforms_to behavior between MEDIA_OBJECT and MEDIA_STRING | src/urn/media_urn.rs:835 |
| test088 | `test088_resolve_from_registry_str` | TEST088: Test resolving string media URN from registry returns correct media type and profile | src/media/spec.rs:635 |
| test089 | `test089_resolve_from_registry_obj` | TEST089: Test resolving JSON media URN from registry returns JSON media type | src/media/spec.rs:645 |
| test090 | `test090_resolve_from_registry_binary` | TEST090: Test resolving binary media URN returns octet-stream and is_binary true | src/media/spec.rs:654 |
| test091 | `test091_resolve_custom_media_spec` | TEST091: Test resolving custom media URN from local media_specs takes precedence over registry | src/media/spec.rs:678 |
| test092 | `test092_resolve_custom_with_schema` | TEST092: Test resolving custom record media spec with schema from local media_specs | src/media/spec.rs:707 |
| test093 | `test093_resolve_unresolvable_fails_hard` | TEST093: Test resolving unknown media URN fails with UnresolvableMediaUrn error | src/media/spec.rs:741 |
| test094 | `test094_local_overrides_registry` | TEST094: Test local media_specs definition overrides registry definition for same URN | src/media/spec.rs:755 |
| test095 | `test095_media_spec_def_serialize` | TEST095: Test MediaSpecDef serializes with required fields and skips None fields | src/media/spec.rs:787 |
| test096 | `test096_media_spec_def_deserialize` | TEST096: Test deserializing MediaSpecDef from JSON object | src/media/spec.rs:812 |
| test097 | `test097_validate_no_duplicate_urns_catches_duplicates` | TEST097: Test duplicate URN validation catches duplicates | src/media/spec.rs:827 |
| test098 | `test098_validate_no_duplicate_urns_passes_for_unique` | TEST098: Test duplicate URN validation passes for unique URNs | src/media/spec.rs:843 |
| test099 | `test099_resolved_is_binary` | TEST099: Test ResolvedMediaSpec is_binary returns true when textable tag is absent | src/media/spec.rs:858 |
| test100 | `test100_resolved_is_record` | TEST100: Test ResolvedMediaSpec is_record returns true when record marker is present | src/media/spec.rs:877 |
| test101 | `test101_resolved_is_scalar` | TEST101: Test ResolvedMediaSpec is_scalar returns true when list marker is absent | src/media/spec.rs:897 |
| test102 | `test102_resolved_is_list` | TEST102: Test ResolvedMediaSpec is_list returns true when list marker is present | src/media/spec.rs:916 |
| test103 | `test103_resolved_is_json` | TEST103: Test ResolvedMediaSpec is_json returns true when json tag is present | src/media/spec.rs:935 |
| test104 | `test104_resolved_is_text` | TEST104: Test ResolvedMediaSpec is_text returns true when textable tag is present | src/media/spec.rs:954 |
| test105 | `test105_metadata_propagation` | TEST105: Test metadata propagates from media spec def to resolved media spec | src/media/spec.rs:977 |
| test106 | `test106_metadata_with_validation` | TEST106: Test metadata and validation can coexist in media spec definition | src/media/spec.rs:1005 |
| test107 | `test107_extensions_propagation` | TEST107: Test extensions field propagates from media spec def to resolved | src/media/spec.rs:1051 |
| test108 | `test108_cap_creation` | TEST108: Test creating new cap with URN, title, and command verifies correct initialization | src/cap/definition.rs:863 |
| test109 | `test109_cap_with_metadata` | TEST109: Test creating cap with metadata initializes and retrieves metadata correctly | src/cap/definition.rs:879 |
| test110 | `test110_cap_matching` | TEST110: Test cap matching with subset semantics for request fulfillment | src/cap/definition.rs:896 |
| test111 | `test111_cap_title` | TEST111: Test getting and setting cap title updates correctly | src/cap/definition.rs:909 |
| test112 | `test112_cap_definition_equality` | TEST112: Test cap equality based on URN and title matching | src/cap/definition.rs:923 |
| test113 | `test113_cap_stdin` | TEST113: Test cap stdin support via args with stdin source and serialization roundtrip | src/cap/definition.rs:938 |
| test114 | `test114_arg_source_types` | TEST114: Test ArgSource type variants stdin, position, and cli_flag with their accessors | src/cap/definition.rs:971 |
| test115 | `test115_cap_arg_serialization` | TEST115: Test CapArg serialization and deserialization with multiple sources | src/cap/definition.rs:996 |
| test116 | `test116_cap_arg_constructors` | TEST116: Test CapArg constructor methods basic and with_description create args correctly | src/cap/definition.rs:1021 |
| test117 | `test117_register_and_find_cap_set` | TEST117: Test registering cap set and finding by exact and subset matching | src/urn/cap_matrix.rs:982 |
| test118 | `test118_best_cap_set_selection` | TEST118: Test selecting best cap set based on specificity ranking  With is_dispatchable semantics: - Provider must satisfy ALL request constraints - General request matches specific provider (provider refines request) - Specific request does NOT match general provider (provider lacks constraints) | src/urn/cap_matrix.rs:1030 |
| test119 | `test119_invalid_urn_handling` | TEST119: Test invalid URN returns InvalidUrn error | src/urn/cap_matrix.rs:1092 |
| test120 | `test120_accepts_request` | TEST120: Test accepts_request checks if registry can handle a capability request | src/urn/cap_matrix.rs:1102 |
| test121 | `test121_cap_block_more_specific_wins` | TEST121: Test CapBlock selects more specific cap over less specific regardless of registry order | src/urn/cap_matrix.rs:1163 |
| test122 | `test122_cap_block_tie_goes_to_first` | TEST122: Test CapBlock breaks specificity ties by first registered registry | src/urn/cap_matrix.rs:1214 |
| test123 | `test123_cap_block_polls_all` | TEST123: Test CapBlock polls all registries to find most specific match | src/urn/cap_matrix.rs:1243 |
| test124 | `test124_cap_block_no_match` | TEST124: Test CapBlock returns error when no registries match the request | src/urn/cap_matrix.rs:1279 |
| test125 | `test125_cap_block_fallback_scenario` | TEST125: Test CapBlock prefers specific plugin over generic provider fallback | src/urn/cap_matrix.rs:1292 |
| test126 | `test126_composite_can_method` | TEST126: Test composite can method returns CapCaller for capability execution | src/urn/cap_matrix.rs:1357 |
| test127 | `test127_cap_graph_basic_construction` | TEST127: Test CapGraph adds nodes and edges from capability definitions | src/urn/cap_matrix.rs:1392 |
| test128 | `test128_cap_graph_outgoing_incoming` | TEST128: Test CapGraph tracks outgoing and incoming edges for spec conversions | src/urn/cap_matrix.rs:1423 |
| test129 | `test129_cap_graph_can_convert` | TEST129: Test CapGraph detects direct and indirect conversion paths between specs | src/urn/cap_matrix.rs:1472 |
| test130 | `test130_cap_graph_find_path` | TEST130: Test CapGraph finds shortest path for spec conversion chain | src/urn/cap_matrix.rs:1525 |
| test131 | `test131_cap_graph_find_all_paths` | TEST131: Test CapGraph finds all conversion paths sorted by length | src/urn/cap_matrix.rs:1581 |
| test132 | `test132_cap_graph_get_direct_edges_sorted` | TEST132: Test CapGraph returns direct edges sorted by specificity | src/urn/cap_matrix.rs:1639 |
| test133 | `test133_cap_block_graph_integration` | TEST133: Test CapBlock graph integration with multiple registries and conversion paths | src/urn/cap_matrix.rs:1681 |
| test134 | `test134_cap_graph_stats` | TEST134: Test CapGraph stats provides counts of nodes and edges | src/urn/cap_matrix.rs:1766 |
| test135 | `test135_registry_creation` | TEST135: Test registry creation with temporary cache directory succeeds | src/cap/registry.rs:637 |
| test136 | `test136_cache_key_generation` | TEST136: Test cache key generation produces consistent hashes for same URN | src/cap/registry.rs:644 |
| test137 | `test137_parse_registry_json` | TEST137: Test parsing registry JSON without stdin args verifies cap structure | src/cap/registry.rs:662 |
| test138 | `test138_parse_registry_json_with_stdin` | TEST138: Test parsing registry JSON with stdin args verifies stdin media URN extraction | src/cap/registry.rs:675 |
| test139 | `test139_url_keeps_cap_prefix_literal` | / Test that URL construction keeps "cap:" literal and only encodes the tags part / This guards against the bug where encoding "cap:" as "cap%3A" causes 404s TEST139: Test URL construction keeps cap prefix literal and only encodes tags part | src/cap/registry.rs:694 |
| test140 | `test140_url_encodes_quoted_media_urns` | / Test that media URNs in cap URNs are properly URL-encoded TEST140: Test URL encodes media URNs with proper percent encoding for special characters | src/cap/registry.rs:710 |
| test141 | `test141_exact_url_format` | / Test the URL format for a simple cap URN TEST141: Test exact URL format contains properly encoded media URN components | src/cap/registry.rs:730 |
| test142 | `test142_normalize_handles_different_tag_orders` | / Test that normalization handles various input formats TEST142: Test normalize handles different tag orders producing same canonical form | src/cap/registry.rs:747 |
| test143 | `test143_default_config` | TEST143: Test default config uses capdag.com or environment variable values | src/cap/registry.rs:765 |
| test144 | `test144_custom_registry_url` | TEST144: Test custom registry URL updates both registry and schema base URLs | src/cap/registry.rs:777 |
| test145 | `test145_custom_registry_and_schema_url` | TEST145: Test custom registry and schema URLs set independently | src/cap/registry.rs:786 |
| test146 | `test146_schema_url_not_overwritten_when_explicit` | TEST146: Test schema URL not overwritten when set explicitly before registry URL | src/cap/registry.rs:796 |
| test147 | `test147_registry_for_test_with_config` | TEST147: Test registry for test with custom config creates registry with specified URLs | src/cap/registry.rs:807 |
| test148 | `test148_cap_manifest_creation` | TEST148: Test creating cap manifest with name, version, description, and caps | src/bifaci/manifest.rs:102 |
| test149 | `test149_cap_manifest_with_author` | TEST149: Test cap manifest with author field sets author correctly | src/bifaci/manifest.rs:122 |
| test150 | `test150_cap_manifest_json_serialization` | TEST150: Test cap manifest JSON serialization and deserialization roundtrip | src/bifaci/manifest.rs:138 |
| test151 | `test151_cap_manifest_required_fields` | TEST151: Test cap manifest deserialization fails when required fields are missing | src/bifaci/manifest.rs:178 |
| test152 | `test152_cap_manifest_with_multiple_caps` | TEST152: Test cap manifest with multiple caps stores and retrieves all capabilities | src/bifaci/manifest.rs:191 |
| test153 | `test153_cap_manifest_empty_caps` | TEST153: Test cap manifest with empty caps list serializes and deserializes correctly | src/bifaci/manifest.rs:218 |
| test154 | `test154_cap_manifest_optional_author_field` | TEST154: Test cap manifest optional author field skipped in serialization when None | src/bifaci/manifest.rs:236 |
| test155 | `test155_component_metadata_trait` | TEST155: Test ComponentMetadata trait provides manifest and caps accessor methods | src/bifaci/manifest.rs:258 |
| test156 | `test156_stdin_source_data_creation` | TEST156: Test creating StdinSource Data variant with byte vector | src/cap/caller.rs:324 |
| test157 | `test157_stdin_source_file_reference_creation` | TEST157: Test creating StdinSource FileReference variant with all required fields | src/cap/caller.rs:336 |
| test158 | `test158_stdin_source_empty_data` | TEST158: Test StdinSource Data with empty vector stores and retrieves correctly | src/cap/caller.rs:367 |
| test159 | `test159_stdin_source_binary_content` | TEST159: Test StdinSource Data with binary content like PNG header bytes | src/cap/caller.rs:378 |
| test160 | `test160_stdin_source_clone` | TEST160: Test StdinSource Data clone creates independent copy with same data | src/cap/caller.rs:396 |
| test161 | `test161_stdin_source_file_reference_clone` | TEST161: Test StdinSource FileReference clone creates independent copy with same fields | src/cap/caller.rs:409 |
| test162 | `test162_stdin_source_debug` | TEST162: Test StdinSource Debug format displays variant type and relevant fields | src/cap/caller.rs:444 |
| test163 | `test163_argument_schema_validation_success` | TEST163: Test argument schema validation succeeds with valid JSON matching schema | src/cap/schema_validation.rs:233 |
| test164 | `test164_argument_schema_validation_failure` | TEST164: Test argument schema validation fails with JSON missing required fields | src/cap/schema_validation.rs:273 |
| test165 | `test165_output_schema_validation_success` | TEST165: Test output schema validation succeeds with valid JSON matching schema | src/cap/schema_validation.rs:312 |
| test166 | `test166_skip_validation_without_schema` | TEST166: Test validation skipped when resolved media spec has no schema | src/cap/schema_validation.rs:348 |
| test167 | `test167_unresolvable_media_urn_fails_hard` | TEST167: Test validation fails hard when media URN cannot be resolved from any source | src/cap/schema_validation.rs:370 |
| test168 | `test168_json_response` | TEST168: Test ResponseWrapper from JSON deserializes to correct structured type | src/cap/response.rs:253 |
| test169 | `test169_primitive_types` | TEST169: Test ResponseWrapper converts to primitive types integer, float, boolean, string | src/cap/response.rs:267 |
| test170 | `test170_binary_response` | TEST170: Test ResponseWrapper from binary stores and retrieves raw bytes correctly | src/cap/response.rs:287 |
| test171 | `test171_frame_type_roundtrip` | TEST171: Test all FrameType discriminants roundtrip through u8 conversion preserving identity | src/bifaci/frame.rs:936 |
| test172 | `test172_invalid_frame_type` | TEST172: Test FrameType::from_u8 returns None for values outside the valid discriminant range | src/bifaci/frame.rs:959 |
| test173 | `test173_frame_type_discriminant_values` | TEST173: Test FrameType discriminant values match the wire protocol specification exactly | src/bifaci/frame.rs:967 |
| test174 | `test174_message_id_uuid` | TEST174: Test MessageId::new_uuid generates valid UUID that roundtrips through string conversion | src/bifaci/frame.rs:984 |
| test175 | `test175_message_id_uuid_uniqueness` | TEST175: Test two MessageId::new_uuid calls produce distinct IDs (no collisions) | src/bifaci/frame.rs:993 |
| test176 | `test176_message_id_uint_has_no_uuid_string` | TEST176: Test MessageId::Uint does not produce a UUID string, to_uuid_string returns None | src/bifaci/frame.rs:1001 |
| test177 | `test177_message_id_from_invalid_uuid_str` | TEST177: Test MessageId::from_uuid_str rejects invalid UUID strings | src/bifaci/frame.rs:1008 |
| test178 | `test178_message_id_as_bytes` | TEST178: Test MessageId::as_bytes produces correct byte representations for Uuid and Uint variants | src/bifaci/frame.rs:1016 |
| test179 | `test179_message_id_default_is_uuid` | TEST179: Test MessageId::default creates a UUID variant (not Uint) | src/bifaci/frame.rs:1029 |
| test180 | `test180_hello_frame` | TEST180: Test Frame::hello without manifest produces correct HELLO frame for host side | src/bifaci/frame.rs:1036 |
| test181 | `test181_hello_frame_with_manifest` | TEST181: Test Frame::hello_with_manifest produces HELLO with manifest bytes for plugin side | src/bifaci/frame.rs:1050 |
| test182 | `test182_req_frame` | TEST182: Test Frame::req stores cap URN, payload, and content_type correctly | src/bifaci/frame.rs:1062 |
| test184 | `test184_chunk_frame` | TEST184: Test Frame::chunk stores seq and payload for streaming (with stream_id) | src/bifaci/frame.rs:1078 |
| test185 | `test185_err_frame` | TEST185: Test Frame::err stores error code and message in metadata | src/bifaci/frame.rs:1094 |
| test186 | `test186_log_frame` | TEST186: Test Frame::log stores level and message in metadata | src/bifaci/frame.rs:1104 |
| test187 | `test187_end_frame_with_payload` | TEST187: Test Frame::end with payload sets eof and optional final payload | src/bifaci/frame.rs:1115 |
| test188 | `test188_end_frame_without_payload` | TEST188: Test Frame::end without payload still sets eof marker | src/bifaci/frame.rs:1125 |
| test189 | `test189_chunk_with_offset` | TEST189: Test chunk_with_offset sets offset on all chunks but len only on seq=0 (with stream_id) | src/bifaci/frame.rs:1135 |
| test190 | `test190_heartbeat_frame` | TEST190: Test Frame::heartbeat creates minimal frame with no payload or metadata | src/bifaci/frame.rs:1161 |
| test191 | `test191_error_accessors_on_non_err_frame` | TEST191: Test error_code and error_message return None for non-Err frame types | src/bifaci/frame.rs:1173 |
| test192 | `test192_log_accessors_on_non_log_frame` | TEST192: Test log_level and log_message return None for non-Log frame types | src/bifaci/frame.rs:1184 |
| test193 | `test193_hello_accessors_on_non_hello_frame` | TEST193: Test hello_max_frame and hello_max_chunk return None for non-Hello frame types | src/bifaci/frame.rs:1192 |
| test194 | `test194_frame_new_defaults` | TEST194: Test Frame::new sets version and defaults correctly, optional fields are None | src/bifaci/frame.rs:1201 |
| test195 | `test195_frame_default` | TEST195: Test Frame::default creates a Req frame (the documented default) | src/bifaci/frame.rs:1219 |
| test196 | `test196_is_eof_when_none` | TEST196: Test is_eof returns false when eof field is None (unset) | src/bifaci/frame.rs:1227 |
| test197 | `test197_is_eof_when_false` | TEST197: Test is_eof returns false when eof field is explicitly Some(false) | src/bifaci/frame.rs:1234 |
| test198 | `test198_limits_default` | TEST198: Test Limits::default provides the documented default values | src/bifaci/frame.rs:1242 |
| test199 | `test199_protocol_version_constant` | TEST199: Test PROTOCOL_VERSION is 2 | src/bifaci/frame.rs:1252 |
| test200 | `test200_key_constants` | TEST200: Test integer key constants match the protocol specification | src/bifaci/frame.rs:1258 |
| test201 | `test201_hello_manifest_binary_data` | TEST201: Test hello_with_manifest preserves binary manifest data (not just JSON text) | src/bifaci/frame.rs:1274 |
| test202 | `test202_message_id_equality_and_hash` | TEST202: Test MessageId Eq/Hash semantics: equal UUIDs are equal, different ones are not | src/bifaci/frame.rs:1282 |
| test203 | `test203_message_id_cross_variant_inequality` | TEST203: Test Uuid and Uint variants of MessageId are never equal even for coincidental byte values | src/bifaci/frame.rs:1305 |
| test204 | `test204_req_frame_empty_payload` | TEST204: Test Frame::req with empty payload stores Some(empty vec) not None | src/bifaci/frame.rs:1313 |
| test205 | `test205_encode_decode_roundtrip` | TEST205: Test REQ frame encode/decode roundtrip preserves all fields | src/bifaci/io.rs:844 |
| test206 | `test206_hello_frame_roundtrip` | TEST206: Test HELLO frame encode/decode roundtrip preserves max_frame, max_chunk, max_reorder_buffer | src/bifaci/io.rs:861 |
| test207 | `test207_err_frame_roundtrip` | TEST207: Test ERR frame encode/decode roundtrip preserves error code and message | src/bifaci/io.rs:874 |
| test208 | `test208_log_frame_roundtrip` | TEST208: Test LOG frame encode/decode roundtrip preserves level and message | src/bifaci/io.rs:887 |
| test210 | `test210_end_frame_roundtrip` | TEST210: Test END frame encode/decode roundtrip preserves eof marker and optional payload | src/bifaci/io.rs:903 |
| test211 | `test211_hello_with_manifest_roundtrip` | TEST211: Test HELLO with manifest encode/decode roundtrip preserves manifest bytes and limits | src/bifaci/io.rs:917 |
| test212 | `test212_chunk_with_offset_roundtrip` | TEST212: Test chunk_with_offset encode/decode roundtrip preserves offset, len, eof (with stream_id) | src/bifaci/io.rs:931 |
| test213 | `test213_heartbeat_roundtrip` | TEST213: Test heartbeat frame encode/decode roundtrip preserves ID with no extra fields | src/bifaci/io.rs:951 |
| test214 | `test214_frame_io_roundtrip` | TEST214: Test write_frame/read_frame IO roundtrip through length-prefixed wire format | src/bifaci/io.rs:965 |
| test215 | `test215_multiple_frames` | TEST215: Test reading multiple sequential frames from a single buffer | src/bifaci/io.rs:988 |
| test216 | `test216_frame_too_large` | TEST216: Test write_frame rejects frames exceeding max_frame limit | src/bifaci/io.rs:1026 |
| test217 | `test217_read_frame_too_large` | TEST217: Test read_frame rejects incoming frames exceeding the negotiated max_frame limit | src/bifaci/io.rs:1044 |
| test218 | `test218_write_chunked` | TEST218: Test write_chunked splits data into chunks respecting max_chunk and reconstructs correctly Chunks from write_chunked have seq=0. SeqAssigner at the output stage assigns final seq. Chunk ordering within a stream is tracked by chunk_index (chunk_index field). | src/bifaci/io.rs:1065 |
| test219 | `test219_write_chunked_empty_data` | TEST219: Test write_chunked with empty data produces a single EOF chunk | src/bifaci/io.rs:1133 |
| test220 | `test220_write_chunked_exact_fit` | TEST220: Test write_chunked with data exactly equal to max_chunk produces exactly one chunk | src/bifaci/io.rs:1151 |
| test221 | `test221_eof_handling` | TEST221: Test read_frame returns Ok(None) on clean EOF (empty stream) | src/bifaci/io.rs:1171 |
| test222 | `test222_truncated_length_prefix` | TEST222: Test read_frame handles truncated length prefix (fewer than 4 bytes available) | src/bifaci/io.rs:1181 |
| test223 | `test223_truncated_frame_body` | TEST223: Test read_frame returns error on truncated frame body (length prefix says more bytes than available) | src/bifaci/io.rs:1200 |
| test224 | `test224_message_id_uint` | TEST224: Test MessageId::Uint roundtrips through encode/decode | src/bifaci/io.rs:1215 |
| test225 | `test225_decode_non_map_value` | TEST225: Test decode_frame rejects non-map CBOR values (e.g., array, integer, string) | src/bifaci/io.rs:1227 |
| test226 | `test226_decode_missing_version` | TEST226: Test decode_frame rejects CBOR map missing required version field | src/bifaci/io.rs:1239 |
| test227 | `test227_decode_invalid_frame_type_value` | TEST227: Test decode_frame rejects CBOR map with invalid frame_type value | src/bifaci/io.rs:1254 |
| test228 | `test228_decode_missing_id` | TEST228: Test decode_frame rejects CBOR map missing required id field | src/bifaci/io.rs:1269 |
| test229 | `test229_frame_reader_writer_set_limits` | TEST229: Test FrameReader/FrameWriter set_limits updates the negotiated limits | src/bifaci/io.rs:1284 |
| test230 | `test230_async_handshake` | TEST230: Test async handshake exchanges HELLO frames and negotiates minimum limits | src/bifaci/io.rs:1301 |
| test231 | `test231_handshake_rejects_non_hello` | TEST231: Test handshake fails when peer sends non-HELLO frame | src/bifaci/io.rs:1331 |
| test232 | `test232_handshake_rejects_missing_manifest` | TEST232: Test handshake fails when plugin HELLO is missing required manifest | src/bifaci/io.rs:1358 |
| test233 | `test233_binary_payload_all_byte_values` | TEST233: Test binary payload with all 256 byte values roundtrips through encode/decode | src/bifaci/io.rs:1381 |
| test234 | `test234_decode_garbage_bytes` | TEST234: Test decode_frame handles garbage CBOR bytes gracefully with an error | src/bifaci/io.rs:1398 |
| test235 | `test235_response_chunk` | TEST235: Test ResponseChunk stores payload, seq, offset, len, and eof fields correctly | src/bifaci/host_runtime.rs:1535 |
| test236 | `test236_response_chunk_with_all_fields` | TEST236: Test ResponseChunk with all fields populated preserves offset, len, and eof | src/bifaci/host_runtime.rs:1551 |
| test237 | `test237_plugin_response_single` | TEST237: Test PluginResponse::Single final_payload returns the single payload slice | src/bifaci/host_runtime.rs:1567 |
| test238 | `test238_plugin_response_single_empty` | TEST238: Test PluginResponse::Single with empty payload returns empty slice and empty vec | src/bifaci/host_runtime.rs:1575 |
| test239 | `test239_plugin_response_streaming` | TEST239: Test PluginResponse::Streaming concatenated joins all chunk payloads in order | src/bifaci/host_runtime.rs:1583 |
| test240 | `test240_plugin_response_streaming_final_payload` | TEST240: Test PluginResponse::Streaming final_payload returns the last chunk's payload | src/bifaci/host_runtime.rs:1594 |
| test241 | `test241_plugin_response_streaming_empty_chunks` | TEST241: Test PluginResponse::Streaming with empty chunks vec returns empty concatenation | src/bifaci/host_runtime.rs:1605 |
| test242 | `test242_plugin_response_streaming_large_payload` | TEST242: Test PluginResponse::Streaming concatenated capacity is pre-allocated correctly for large payloads | src/bifaci/host_runtime.rs:1613 |
| test243 | `test243_async_host_error_display` | TEST243: Test AsyncHostError variants display correct error messages | src/bifaci/host_runtime.rs:1629 |
| test244 | `test244_async_host_error_from_cbor` | TEST244: Test AsyncHostError::from converts CborError to Cbor variant | src/bifaci/host_runtime.rs:1643 |
| test245 | `test245_async_host_error_from_io` | TEST245: Test AsyncHostError::from converts io::Error to Io variant | src/bifaci/host_runtime.rs:1654 |
| test246 | `test246_async_host_error_clone` | TEST246: Test AsyncHostError Clone implementation produces equal values | src/bifaci/host_runtime.rs:1665 |
| test247 | `test247_response_chunk_clone` | TEST247: Test ResponseChunk Clone produces independent copy with same data | src/bifaci/host_runtime.rs:1673 |
| test248 | `test248_register_and_find_handler` | TEST248: Test register_op and find_handler by exact cap URN | src/bifaci/plugin_runtime.rs:3296 |
| test249 | `test249_raw_handler` | TEST249: Test register_op handler echoes bytes directly | src/bifaci/plugin_runtime.rs:3304 |
| test250 | `test250_typed_handler_deserialization` | TEST250: Test Op handler collects input and processes it | src/bifaci/plugin_runtime.rs:3322 |
| test251 | `test251_typed_handler_rejects_invalid_json` | TEST251: Test Op handler propagates errors through RuntimeError::Handler | src/bifaci/plugin_runtime.rs:3365 |
| test252 | `test252_find_handler_unknown_cap` | TEST252: Test find_handler returns None for unregistered cap URNs | src/bifaci/plugin_runtime.rs:3398 |
| test253 | `test253_handler_is_send_sync` | TEST253: Test OpFactory can be cloned via Arc and sent across tasks (Send + Sync) | src/bifaci/plugin_runtime.rs:3405 |
| test254 | `test254_no_peer_invoker` | TEST254: Test NoPeerInvoker always returns PeerRequest error | src/bifaci/plugin_runtime.rs:3450 |
| test255 | `test255_no_peer_invoker_with_arguments` | TEST255: Test NoPeerInvoker call_with_bytes also returns error | src/bifaci/plugin_runtime.rs:3464 |
| test256 | `test256_with_manifest_json` | TEST256: Test PluginRuntime::with_manifest_json stores manifest data and parses when valid | src/bifaci/plugin_runtime.rs:3481 |
| test257 | `test257_new_with_invalid_json` | TEST257: Test PluginRuntime::new with invalid JSON still creates runtime (manifest is None) | src/bifaci/plugin_runtime.rs:3498 |
| test258 | `test258_with_manifest_struct` | TEST258: Test PluginRuntime::with_manifest creates runtime with valid manifest data | src/bifaci/plugin_runtime.rs:3506 |
| test259 | `test259_extract_effective_payload_non_cbor` | TEST259: Test extract_effective_payload with non-CBOR content_type returns raw payload unchanged | src/bifaci/plugin_runtime.rs:3515 |
| test260 | `test260_extract_effective_payload_no_content_type` | TEST260: Test extract_effective_payload with None content_type returns raw payload unchanged | src/bifaci/plugin_runtime.rs:3525 |
| test261 | `test261_extract_effective_payload_cbor_match` | TEST261: Test extract_effective_payload with CBOR content extracts matching argument value | src/bifaci/plugin_runtime.rs:3535 |
| test262 | `test262_extract_effective_payload_cbor_no_match` | TEST262: Test extract_effective_payload with CBOR content fails when no argument matches expected input | src/bifaci/plugin_runtime.rs:3583 |
| test263 | `test263_extract_effective_payload_invalid_cbor` | TEST263: Test extract_effective_payload with invalid CBOR bytes returns deserialization error | src/bifaci/plugin_runtime.rs:3612 |
| test264 | `test264_extract_effective_payload_cbor_not_array` | TEST264: Test extract_effective_payload with CBOR non-array (e.g. map) returns error | src/bifaci/plugin_runtime.rs:3626 |
| test266 | `test266_cli_frame_sender_construction` | TEST266: Test CliFrameSender wraps CliStreamEmitter correctly (basic construction) | src/bifaci/plugin_runtime.rs:3650 |
| test268 | `test268_runtime_error_display` | TEST268: Test RuntimeError variants display correct messages | src/bifaci/plugin_runtime.rs:3661 |
| test270 | `test270_multiple_handlers` | TEST270: Test registering multiple Op handlers for different caps and finding each independently | src/bifaci/plugin_runtime.rs:3683 |
| test271 | `test271_handler_replacement` | TEST271: Test Op handler replacing an existing registration for the same cap URN | src/bifaci/plugin_runtime.rs:3708 |
| test272 | `test272_extract_effective_payload_multiple_args` | TEST272: Test extract_effective_payload CBOR with multiple arguments selects the correct one | src/bifaci/plugin_runtime.rs:3760 |
| test273 | `test273_extract_effective_payload_binary_value` | TEST273: Test extract_effective_payload with binary data in CBOR value (not just text) | src/bifaci/plugin_runtime.rs:3831 |
| test274 | `test274_cap_argument_value_new` | TEST274: Test CapArgumentValue::new stores media_urn and raw byte value | src/cap/caller.rs:463 |
| test275 | `test275_cap_argument_value_from_str` | TEST275: Test CapArgumentValue::from_str converts string to UTF-8 bytes | src/cap/caller.rs:471 |
| test276 | `test276_cap_argument_value_as_str_valid` | TEST276: Test CapArgumentValue::value_as_str succeeds for UTF-8 data | src/cap/caller.rs:479 |
| test277 | `test277_cap_argument_value_as_str_invalid_utf8` | TEST277: Test CapArgumentValue::value_as_str fails for non-UTF-8 binary data | src/cap/caller.rs:486 |
| test278 | `test278_cap_argument_value_empty` | TEST278: Test CapArgumentValue::new with empty value stores empty vec | src/cap/caller.rs:493 |
| test279 | `test279_cap_argument_value_clone` | TEST279: Test CapArgumentValue Clone produces independent copy with same data | src/cap/caller.rs:501 |
| test280 | `test280_cap_argument_value_debug` | TEST280: Test CapArgumentValue Debug format includes media_urn and value | src/cap/caller.rs:510 |
| test281 | `test281_cap_argument_value_into_string` | TEST281: Test CapArgumentValue::new accepts Into<String> for media_urn (String and &str) | src/cap/caller.rs:518 |
| test282 | `test282_cap_argument_value_unicode` | TEST282: Test CapArgumentValue::from_str with Unicode string preserves all characters | src/cap/caller.rs:529 |
| test283 | `test283_cap_argument_value_large_binary` | TEST283: Test CapArgumentValue with large binary payload preserves all bytes | src/cap/caller.rs:536 |
| test284 | `test284_handshake_host_plugin` | TEST284: Handshake exchanges HELLO frames, negotiates limits | src/bifaci/integration_tests.rs:776 |
| test285 | `test285_request_response_simple` | TEST285: Simple request-response flow (REQ → END with payload) | src/bifaci/integration_tests.rs:811 |
| test286 | `test286_streaming_chunks` | TEST286: Streaming response with multiple CHUNK frames | src/bifaci/integration_tests.rs:853 |
| test287 | `test287_heartbeat_from_host` | TEST287: Host-initiated heartbeat | src/bifaci/integration_tests.rs:919 |
| test290 | `test290_limits_negotiation` | TEST290: Limit negotiation picks minimum | src/bifaci/integration_tests.rs:958 |
| test291 | `test291_binary_payload_roundtrip` | TEST291: Binary payload roundtrip (all 256 byte values) | src/bifaci/integration_tests.rs:986 |
| test292 | `test292_message_id_uniqueness` | TEST292: Sequential requests get distinct MessageIds | src/bifaci/integration_tests.rs:1038 |
| test293 | `test293_plugin_runtime_handler_registration` | TEST293: Test PluginRuntime Op registration and lookup by exact and non-existent cap URN | src/bifaci/integration_tests.rs:21 |
| test299 | `test299_empty_payload_roundtrip` | TEST299: Empty payload request/response roundtrip | src/bifaci/integration_tests.rs:1091 |
| test304 | `test304_media_availability_output_constant` | TEST304: Test MEDIA_AVAILABILITY_OUTPUT constant parses as valid media URN with correct tags | src/urn/media_urn.rs:849 |
| test305 | `test305_media_path_output_constant` | TEST305: Test MEDIA_PATH_OUTPUT constant parses as valid media URN with correct tags | src/urn/media_urn.rs:861 |
| test306 | `test306_availability_and_path_output_distinct` | TEST306: Test MEDIA_AVAILABILITY_OUTPUT and MEDIA_PATH_OUTPUT are distinct URNs | src/urn/media_urn.rs:872 |
| test307 | `test307_model_availability_urn` | TEST307: Test model_availability_urn builds valid cap URN with correct op and media specs | src/standard/caps.rs:720 |
| test308 | `test308_model_path_urn` | TEST308: Test model_path_urn builds valid cap URN with correct op and media specs | src/standard/caps.rs:729 |
| test309 | `test309_model_availability_and_path_are_distinct` | TEST309: Test model_availability_urn and model_path_urn produce distinct URNs | src/standard/caps.rs:738 |
| test310 | `test310_llm_conversation_urn_unconstrained` | TEST310: Test llm_conversation_urn uses unconstrained tag (not constrained) | src/standard/caps.rs:747 |
| test311 | `test311_llm_conversation_urn_specs` | TEST311: Test llm_conversation_urn in/out specs match the expected media URNs semantically | src/standard/caps.rs:756 |
| test312 | `test312_all_urn_builders_produce_valid_urns` | TEST312: Test all URN builders produce parseable cap URNs | src/standard/caps.rs:774 |
| test320 | `test320_plugin_info_construction` | TEST320-335: PluginRepoServer and PluginRepoClient tests | src/bifaci/plugin_repo.rs:724 |
| test321 | `test321_plugin_info_is_signed` |  | src/bifaci/plugin_repo.rs:757 |
| test322 | `test322_plugin_info_has_binary` |  | src/bifaci/plugin_repo.rs:795 |
| test323 | `test323_plugin_repo_server_validate_registry` |  | src/bifaci/plugin_repo.rs:833 |
| test324 | `test324_plugin_repo_server_transform_to_array` |  | src/bifaci/plugin_repo.rs:857 |
| test325 | `test325_plugin_repo_server_get_plugins` |  | src/bifaci/plugin_repo.rs:912 |
| test326 | `test326_plugin_repo_server_get_plugin_by_id` |  | src/bifaci/plugin_repo.rs:961 |
| test327 | `test327_plugin_repo_server_search_plugins` |  | src/bifaci/plugin_repo.rs:1013 |
| test328 | `test328_plugin_repo_server_get_by_category` |  | src/bifaci/plugin_repo.rs:1065 |
| test329 | `test329_plugin_repo_server_get_by_cap` |  | src/bifaci/plugin_repo.rs:1117 |
| test330 | `test330_plugin_repo_client_update_cache` |  | src/bifaci/plugin_repo.rs:1174 |
| test331 | `test331_plugin_repo_client_get_suggestions` |  | src/bifaci/plugin_repo.rs:1220 |
| test332 | `test332_plugin_repo_client_get_plugin` |  | src/bifaci/plugin_repo.rs:1269 |
| test333 | `test333_plugin_repo_client_get_all_caps` |  | src/bifaci/plugin_repo.rs:1315 |
| test334 | `test334_plugin_repo_client_needs_sync` |  | src/bifaci/plugin_repo.rs:1394 |
| test335 | `test335_plugin_repo_server_client_integration` |  | src/bifaci/plugin_repo.rs:1413 |
| test336 | `test336_file_path_reads_file_passes_bytes` | TEST336: Single file-path arg with stdin source reads file and passes bytes to handler | src/bifaci/plugin_runtime.rs:3877 |
| test337 | `test337_file_path_without_stdin_passes_string` | TEST337: file-path arg without stdin source passes path as string (no conversion) | src/bifaci/plugin_runtime.rs:3942 |
| test338 | `test338_file_path_via_cli_flag` | TEST338: file-path arg reads file via --file CLI flag | src/bifaci/plugin_runtime.rs:3974 |
| test339 | `test339_file_path_array_glob_expansion` | TEST339: file-path-array reads multiple files with glob pattern | src/bifaci/plugin_runtime.rs:4006 |
| test340 | `test340_file_not_found_clear_error` | TEST340: File not found error provides clear message | src/bifaci/plugin_runtime.rs:4049 |
| test341 | `test341_stdin_precedence_over_file_path` | TEST341: stdin takes precedence over file-path in source order | src/bifaci/plugin_runtime.rs:4090 |
| test342 | `test342_file_path_position_zero_reads_first_arg` | TEST342: file-path with position 0 reads first positional arg as file | src/bifaci/plugin_runtime.rs:4128 |
| test343 | `test343_non_file_path_args_unaffected` | TEST343: Non-file-path args are not affected by file reading | src/bifaci/plugin_runtime.rs:4161 |
| test344 | `test344_file_path_array_invalid_json_fails` | TEST344: file-path-array with nonexistent path fails clearly | src/bifaci/plugin_runtime.rs:4192 |
| test345 | `test345_file_path_array_one_file_missing_fails_hard` | TEST345: file-path-array with literal nonexistent path fails hard | src/bifaci/plugin_runtime.rs:4233 |
| test346 | `test346_large_file_reads_successfully` | TEST346: Large file (1MB) reads successfully | src/bifaci/plugin_runtime.rs:4277 |
| test347 | `test347_empty_file_reads_as_empty_bytes` | TEST347: Empty file reads as empty bytes | src/bifaci/plugin_runtime.rs:4313 |
| test348 | `test348_file_path_conversion_respects_source_order` | TEST348: file-path conversion respects source order | src/bifaci/plugin_runtime.rs:4345 |
| test349 | `test349_file_path_multiple_sources_fallback` | TEST349: file-path arg with multiple sources tries all in order | src/bifaci/plugin_runtime.rs:4382 |
| test350 | `test350_full_cli_mode_with_file_path_integration` | TEST350: Integration test - full CLI mode invocation with file-path | src/bifaci/plugin_runtime.rs:4419 |
| test351 | `test351_file_path_array_empty_array` | TEST351: file-path array with empty CBOR array returns empty (CBOR mode) | src/bifaci/plugin_runtime.rs:4483 |
| test352 | `test352_file_permission_denied_clear_error` |  | src/bifaci/plugin_runtime.rs:4533 |
| test353 | `test353_cbor_payload_format_consistency` | TEST353: CBOR payload format matches between CLI and CBOR mode | src/bifaci/plugin_runtime.rs:4601 |
| test354 | `test354_glob_pattern_no_matches_empty_array` | TEST354: Glob pattern with no matches fails hard (NO FALLBACK) | src/bifaci/plugin_runtime.rs:4665 |
| test355 | `test355_glob_pattern_skips_directories` | TEST355: Glob pattern skips directories | src/bifaci/plugin_runtime.rs:4708 |
| test356 | `test356_multiple_glob_patterns_combined` | TEST356: Multiple glob patterns combined | src/bifaci/plugin_runtime.rs:4752 |
| test357 | `test357_symlinks_followed` |  | src/bifaci/plugin_runtime.rs:4836 |
| test358 | `test358_binary_file_non_utf8` | TEST358: Binary file with non-UTF8 data reads correctly | src/bifaci/plugin_runtime.rs:4879 |
| test359 | `test359_invalid_glob_pattern_fails` | TEST359: Invalid glob pattern fails with clear error | src/bifaci/plugin_runtime.rs:4914 |
| test360 | `test360_extract_effective_payload_with_file_data` | TEST360: Extract effective payload handles file-path data correctly | src/bifaci/plugin_runtime.rs:4956 |
| test361 | `test361_cli_mode_file_path` | TEST361: CLI mode with file path - pass file path as command-line argument | src/bifaci/plugin_runtime.rs:5042 |
| test362 | `test362_cli_mode_piped_binary` | TEST362: CLI mode with binary piped in - pipe binary data via stdin  This test simulates real-world conditions: - Pure binary data piped to stdin (NOT CBOR) - CLI mode detected (command arg present) - Cap accepts stdin source - Binary is chunked on-the-fly and accumulated - Handler receives complete CBOR payload | src/bifaci/plugin_runtime.rs:5088 |
| test363 | `test363_cbor_mode_chunked_content` | TEST363: CBOR mode with chunked content - send file content streaming as chunks | src/bifaci/plugin_runtime.rs:5155 |
| test364 | `test364_cbor_mode_file_path` | TEST364: CBOR mode with file path - send file path in CBOR arguments (auto-conversion) | src/bifaci/plugin_runtime.rs:5201 |
| test365 | `test365_stream_start_frame` | TEST365: Frame::stream_start stores request_id, stream_id, and media_urn | src/bifaci/frame.rs:1320 |
| test366 | `test366_stream_end_frame` | TEST366: Frame::stream_end stores request_id and stream_id | src/bifaci/frame.rs:1337 |
| test367 | `test367_stream_start_with_empty_stream_id` | TEST367: StreamStart frame with empty stream_id still constructs (validation happens elsewhere) | src/bifaci/frame.rs:1353 |
| test368 | `test368_stream_start_with_empty_media_urn` | TEST368: StreamStart frame with empty media_urn still constructs (validation happens elsewhere) | src/bifaci/frame.rs:1364 |
| test389 | `test389_stream_start_roundtrip` | TEST389: StreamStart encode/decode roundtrip preserves stream_id and media_urn | src/bifaci/io.rs:1438 |
| test390 | `test390_stream_end_roundtrip` | TEST390: StreamEnd encode/decode roundtrip preserves stream_id, no media_urn | src/bifaci/io.rs:1455 |
| test395 | `test395_build_payload_small` | TEST395: Small payload (< max_chunk) produces correct CBOR arguments | src/bifaci/plugin_runtime.rs:5361 |
| test396 | `test396_build_payload_large` | TEST396: Large payload (> max_chunk) accumulates across chunks correctly | src/bifaci/plugin_runtime.rs:5404 |
| test397 | `test397_build_payload_empty` | TEST397: Empty reader produces valid empty CBOR arguments | src/bifaci/plugin_runtime.rs:5445 |
| test398 | `test398_build_payload_io_error` | TEST398: IO error from reader propagates as RuntimeError::Io | src/bifaci/plugin_runtime.rs:5483 |
| test399 | `test399_relay_notify_discriminant_roundtrip` | TEST399: Verify RelayNotify frame type discriminant roundtrips through u8 (value 10) | src/bifaci/frame.rs:1375 |
| test400 | `test400_relay_state_discriminant_roundtrip` | TEST400: Verify RelayState frame type discriminant roundtrips through u8 (value 11) | src/bifaci/frame.rs:1384 |
| test401 | `test401_relay_notify_frame` | TEST401: Verify relay_notify factory stores manifest and limits, and accessors extract them | src/bifaci/frame.rs:1393 |
| test402 | `test402_relay_state_frame` | TEST402: Verify relay_state factory stores resource payload in frame payload field | src/bifaci/frame.rs:1408 |
| test403 | `test403_invalid_frame_type_past_relay_state` | TEST403: Verify from_u8 returns None for value 12 (one past RelayState) | src/bifaci/frame.rs:1420 |
| test404 | `test404_slave_sends_relay_notify_on_connect` | TEST404: Slave sends RelayNotify on connect (initial_notify parameter) | src/bifaci/relay.rs:367 |
| test405 | `test405_master_reads_relay_notify` | TEST405: Master reads RelayNotify and extracts manifest + limits | src/bifaci/relay.rs:402 |
| test406 | `test406_slave_stores_relay_state` | TEST406: Slave stores RelayState from master | src/bifaci/relay.rs:432 |
| test407 | `test407_protocol_frames_pass_through` | TEST407: Protocol frames pass through slave transparently (both directions) | src/bifaci/relay.rs:475 |
| test408 | `test408_relay_frames_not_forwarded` | TEST408: RelayNotify/RelayState are NOT forwarded through relay | src/bifaci/relay.rs:567 |
| test409 | `test409_slave_injects_relay_notify_midstream` | TEST409: Slave can inject RelayNotify mid-stream (cap change) | src/bifaci/relay.rs:619 |
| test410 | `test410_master_receives_updated_relay_notify` | TEST410: Master receives updated RelayNotify (cap change callback via read_frame) | src/bifaci/relay.rs:694 |
| test411 | `test411_socket_close_detection` | TEST411: Socket close detection (both directions) | src/bifaci/relay.rs:765 |
| test412 | `test412_bidirectional_concurrent_flow` | TEST412: Bidirectional concurrent frame flow through relay | src/bifaci/relay.rs:806 |
| test413 | `test413_register_plugin_adds_to_cap_table` | TEST413: Register plugin adds entries to cap_table | src/bifaci/host_runtime.rs:1685 |
| test414 | `test414_capabilities_empty_initially` | TEST414: capabilities() returns empty JSON initially (no running plugins) | src/bifaci/host_runtime.rs:1703 |
| test415 | `test415_req_for_known_cap_triggers_spawn` | TEST415: REQ for known cap triggers spawn attempt (verified by expected spawn error for non-existent binary) | src/bifaci/host_runtime.rs:1716 |
| test416 | `test416_attach_plugin_handshake_updates_capabilities` | TEST416: Attach plugin performs HELLO handshake, extracts manifest, updates capabilities | src/bifaci/host_runtime.rs:1771 |
| test417 | `test417_route_req_to_correct_plugin` | TEST417: Route REQ to correct plugin by cap_urn (with two attached plugins) | src/bifaci/host_runtime.rs:1809 |
| test418 | `test418_route_continuation_frames_by_req_id` | TEST418: Route STREAM_START/CHUNK/STREAM_END/END by req_id (not cap_urn) Verifies that after the initial REQ→plugin routing, all subsequent continuation frames with the same req_id are routed to the same plugin — even though no cap_urn is present on those frames. | src/bifaci/host_runtime.rs:2134 |
| test419 | `test419_plugin_heartbeat_handled_locally` | TEST419: Plugin HEARTBEAT handled locally (not forwarded to relay) | src/bifaci/host_runtime.rs:1939 |
| test420 | `test420_plugin_frames_forwarded_to_relay` | TEST420: Plugin non-HELLO/non-HB frames forwarded to relay (pass-through) | src/bifaci/host_runtime.rs:2010 |
| test421 | `test421_plugin_death_updates_capabilities` | TEST421: Plugin death updates capability list (caps removed) | src/bifaci/host_runtime.rs:2265 |
| test422 | `test422_plugin_death_sends_err_for_pending_requests` |  | src/bifaci/host_runtime.rs:2341 |
| test423 | `test423_multiple_plugins_route_independently` | TEST423: Multiple plugins registered with distinct caps route independently | src/bifaci/host_runtime.rs:2415 |
| test424 | `test424_concurrent_requests_to_same_plugin` | TEST424: Concurrent requests to the same plugin are handled independently | src/bifaci/host_runtime.rs:2560 |
| test425 | `test425_find_plugin_for_cap_unknown` | TEST425: find_plugin_for_cap returns None for unregistered cap | src/bifaci/host_runtime.rs:2687 |
| test426 | `test426_single_master_req_response` | TEST426: Single master REQ/response routing | src/bifaci/relay_switch.rs:1813 |
| test427 | `test427_multi_master_cap_routing` | TEST427: Multi-master cap routing | src/bifaci/relay_switch.rs:1859 |
| test428 | `test428_unknown_cap_returns_error` | TEST428: Unknown cap returns error | src/bifaci/relay_switch.rs:1926 |
| test429 | `test429_find_master_for_cap` | TEST429: Cap routing logic (find_master_for_cap) | src/bifaci/relay_switch.rs:1781 |
| test430 | `test430_tie_breaking_same_cap_multiple_masters` | TEST430: Tie-breaking (same cap on multiple masters - first match wins, routing is consistent) | src/bifaci/relay_switch.rs:1944 |
| test431 | `test431_continuation_frame_routing` | TEST431: Continuation frame routing (CHUNK, END follow REQ) | src/bifaci/relay_switch.rs:2013 |
| test432 | `test432_empty_masters_allowed` | TEST432: Empty masters list creates empty switch, add_master works | src/bifaci/relay_switch.rs:2059 |
| test433 | `test433_capability_aggregation_deduplicates` | TEST433: Capability aggregation deduplicates caps | src/bifaci/relay_switch.rs:2072 |
| test434 | `test434_limits_negotiation_minimum` | TEST434: Limits negotiation takes minimum | src/bifaci/relay_switch.rs:2102 |
| test435 | `test435_urn_matching_exact_and_accepts` | TEST435: URN matching (exact vs accepts()) | src/bifaci/relay_switch.rs:2127 |
| test436 | `test436_compute_checksum` | TEST436: Verify FNV-1a checksum function produces consistent results | src/bifaci/frame.rs:1427 |
| test437 | `test437_preferred_cap_routes_to_generic` | TEST437: find_master_for_cap with preferred_cap routes to generic handler  With is_dispatchable semantics: - Generic provider (in=media:) CAN dispatch specific request (in="media:pdf") because media: (wildcard) accepts any input type - Preference routes to preferred among dispatchable candidates | src/bifaci/relay_switch.rs:2190 |
| test438 | `test438_preferred_cap_falls_back_when_not_comparable` | TEST438: find_master_for_cap with preference falls back to closest-specificity when preferred cap is not in the comparable set | src/bifaci/relay_switch.rs:2228 |
| test439 | `test439_generic_provider_can_dispatch_specific_request` | TEST439: Generic provider CAN dispatch specific request (but only matches if no more specific provider exists)  With is_dispatchable: generic provider (in=media:) CAN handle specific request (in="media:pdf") because media: accepts any input type. With preference, can route to generic even when more specific exists. | src/bifaci/relay_switch.rs:2255 |
| test440 | `test440_chunk_index_checksum_roundtrip` | TEST440: CHUNK frame with chunk_index and checksum roundtrips through encode/decode | src/bifaci/io.rs:1501 |
| test441 | `test441_stream_end_chunk_count_roundtrip` | TEST441: STREAM_END frame with chunk_count roundtrips through encode/decode | src/bifaci/io.rs:1523 |
| test442 | `test442_seq_assigner_monotonic_same_rid` | TEST442: SeqAssigner assigns seq 0,1,2,3 for consecutive frames with same RID | src/bifaci/frame.rs:1487 |
| test443 | `test443_seq_assigner_independent_rids` | TEST443: SeqAssigner maintains independent counters for different RIDs | src/bifaci/frame.rs:1509 |
| test444 | `test444_seq_assigner_skips_non_flow` | TEST444: SeqAssigner skips non-flow frames (Heartbeat, RelayNotify, RelayState, Hello) | src/bifaci/frame.rs:1535 |
| test445 | `test445_seq_assigner_remove_by_flow_key` | TEST445: SeqAssigner.remove with FlowKey(rid, None) resets that flow; FlowKey(rid, Some(xid)) is unaffected | src/bifaci/frame.rs:1556 |
| test446 | `test446_seq_assigner_mixed_types` | TEST446: SeqAssigner handles mixed frame types (REQ, CHUNK, LOG, END) for same RID | src/bifaci/frame.rs:1627 |
| test447 | `test447_flow_key_with_xid` | TEST447: FlowKey::from_frame extracts (rid, Some(xid)) when routing_id present | src/bifaci/frame.rs:1653 |
| test448 | `test448_flow_key_without_xid` | TEST448: FlowKey::from_frame extracts (rid, None) when routing_id absent | src/bifaci/frame.rs:1666 |
| test449 | `test449_flow_key_equality` | TEST449: FlowKey equality: same rid+xid equal, different xid different key | src/bifaci/frame.rs:1677 |
| test450 | `test450_flow_key_hash_lookup` | TEST450: FlowKey hash: same keys hash equal (HashMap lookup) | src/bifaci/frame.rs:1694 |
| test451 | `test451_reorder_buffer_in_order` | TEST451: ReorderBuffer in-order delivery: seq 0,1,2 delivered immediately | src/bifaci/frame.rs:1720 |
| test452 | `test452_reorder_buffer_out_of_order` | TEST452: ReorderBuffer out-of-order: seq 1 then 0 delivers both in order | src/bifaci/frame.rs:1739 |
| test453 | `test453_reorder_buffer_gap_fill` | TEST453: ReorderBuffer gap fill: seq 0,2,1 delivers 0, buffers 2, then delivers 1+2 | src/bifaci/frame.rs:1754 |
| test454 | `test454_reorder_buffer_stale_seq` | TEST454: ReorderBuffer stale seq is hard error | src/bifaci/frame.rs:1772 |
| test455 | `test455_reorder_buffer_overflow` | TEST455: ReorderBuffer overflow triggers protocol error | src/bifaci/frame.rs:1787 |
| test456 | `test456_reorder_buffer_independent_flows` | TEST456: Multiple concurrent flows reorder independently | src/bifaci/frame.rs:1803 |
| test457 | `test457_reorder_buffer_cleanup` | TEST457: cleanup_flow removes state; new frames start at seq 0 | src/bifaci/frame.rs:1826 |
| test458 | `test458_reorder_buffer_non_flow_bypass` | TEST458: Non-flow frames bypass reorder entirely | src/bifaci/frame.rs:1843 |
| test459 | `test459_reorder_buffer_end_frame` | TEST459: Terminal END frame flows through correctly | src/bifaci/frame.rs:1859 |
| test460 | `test460_reorder_buffer_err_frame` | TEST460: Terminal ERR frame flows through correctly | src/bifaci/frame.rs:1877 |
| test461 | `test461_write_chunked_seq_zero` | TEST461: write_chunked produces frames with seq=0; SeqAssigner assigns at output stage | src/bifaci/io.rs:1540 |
| test472 | `test472_handshake_negotiates_reorder_buffer` | TEST472: Handshake negotiates max_reorder_buffer (minimum of both sides) | src/bifaci/io.rs:1580 |
| test473 | `test473_cap_discard_parses_as_valid_urn` | TEST473: CAP_DISCARD parses as valid CapUrn with in=media: and out=media:void | src/standard/caps.rs:792 |
| test474 | `test474_cap_discard_accepts_specific_void_cap` | TEST474: CAP_DISCARD accepts specific-input/void-output caps | src/standard/caps.rs:803 |
| test475 | `test475_validate_passes_with_identity` | TEST475: CapManifest::validate() passes when CAP_IDENTITY is present | src/bifaci/manifest.rs:295 |
| test476 | `test476_validate_fails_without_identity` | TEST476: CapManifest::validate() fails when CAP_IDENTITY is missing | src/bifaci/manifest.rs:309 |
| test478 | `test478_auto_registers_identity_handler` | TEST478: PluginRuntime auto-registers identity and discard handlers on construction | src/bifaci/plugin_runtime.rs:5513 |
| test479 | `test479_custom_identity_overrides_default` | TEST479: Custom identity Op overrides auto-registered default | src/bifaci/plugin_runtime.rs:5532 |
| test480 | `test480_parse_caps_rejects_manifest_without_identity` | TEST480: parse_caps_from_manifest rejects manifest without CAP_IDENTITY | src/bifaci/host_runtime.rs:1517 |
| test481 | `test481_verify_identity_succeeds` | TEST481: verify_identity succeeds with standard identity echo handler | src/bifaci/io.rs:1676 |
| test482 | `test482_verify_identity_fails_on_err` | TEST482: verify_identity fails when plugin returns ERR on identity call | src/bifaci/io.rs:1699 |
| test483 | `test483_verify_identity_fails_on_close` | TEST483: verify_identity fails when connection closes before response | src/bifaci/io.rs:1733 |
| test485 | `test485_attach_plugin_identity_verification_succeeds` | TEST485: attach_plugin completes identity verification with working plugin | src/bifaci/host_runtime.rs:2700 |
| test486 | `test486_attach_plugin_identity_verification_fails` | TEST486: attach_plugin rejects plugin that fails identity verification | src/bifaci/host_runtime.rs:2731 |
| test487 | `test487_relay_switch_identity_verification_succeeds` | TEST487: RelaySwitch construction verifies identity through relay chain | src/bifaci/relay_switch.rs:2285 |
| test488 | `test488_relay_switch_identity_verification_fails` | TEST488: RelaySwitch construction fails when master's identity verification fails | src/bifaci/relay_switch.rs:2303 |
| test489 | `test489_add_master_dynamic` | TEST489: add_master dynamically connects new host to running switch | src/bifaci/relay_switch.rs:2469 |
| test490 | `test490_identity_verification_multiple_plugins` | TEST490: Identity verification with multiple plugins through single relay  Both plugins must pass identity verification independently before any real requests are routed. | src/bifaci/integration_tests.rs:1256 |
| test491 | `test491_chunk_requires_chunk_index_and_checksum` | TEST491: Frame::chunk constructor requires and sets chunk_index and checksum | src/bifaci/frame.rs:1899 |
| test492 | `test492_stream_end_requires_chunk_count` | TEST492: Frame::stream_end constructor requires and sets chunk_count | src/bifaci/frame.rs:1914 |
| test493 | `test493_compute_checksum_fnv1a_test_vectors` | TEST493: compute_checksum produces correct FNV-1a hash for known test vectors | src/bifaci/frame.rs:1926 |
| test494 | `test494_compute_checksum_deterministic` | TEST494: compute_checksum is deterministic | src/bifaci/frame.rs:1935 |
| test495 | `test495_cbor_rejects_chunk_without_chunk_index` | TEST495: CBOR decode REJECTS CHUNK frame missing chunk_index field | src/bifaci/frame.rs:1947 |
| test496 | `test496_cbor_rejects_chunk_without_checksum` | TEST496: CBOR decode REJECTS CHUNK frame missing checksum field | src/bifaci/frame.rs:1973 |
| test497 | `test497_chunk_corrupted_payload_rejected` | TEST497: Verify CHUNK frame with corrupted payload is rejected by checksum | src/bifaci/io.rs:1471 |
| test498 | `test498_routing_id_cbor_roundtrip` | TEST498: routing_id field roundtrips through CBOR encoding | src/bifaci/frame.rs:2020 |
| test499 | `test499_chunk_index_checksum_cbor_roundtrip` | TEST499: chunk_index and checksum roundtrip through CBOR encoding | src/bifaci/frame.rs:2038 |
| test500 | `test500_chunk_count_cbor_roundtrip` | TEST500: chunk_count roundtrips through CBOR encoding | src/bifaci/frame.rs:2057 |
| test501 | `test501_frame_new_initializes_optional_fields_none` | TEST501: Frame::new initializes new fields to None | src/bifaci/frame.rs:2073 |
| test502 | `test502_keys_module_new_field_constants` | TEST502: Keys module has constants for new fields | src/bifaci/frame.rs:2084 |
| test503 | `test503_compute_checksum_empty_data` | TEST503: compute_checksum handles empty data correctly | src/bifaci/frame.rs:2093 |
| test504 | `test504_compute_checksum_large_payload` | TEST504: compute_checksum handles large payloads without overflow | src/bifaci/frame.rs:2100 |
| test505 | `test505_chunk_with_offset_sets_chunk_index` | TEST505: chunk_with_offset sets chunk_index correctly | src/bifaci/frame.rs:2112 |
| test506 | `test506_compute_checksum_different_data_different_hash` | TEST506: Different data produces different checksums | src/bifaci/frame.rs:2136 |
| test507 | `test507_reorder_buffer_xid_isolation` | TEST507: ReorderBuffer isolates flows by XID (routing_id) - same RID different XIDs | src/bifaci/frame.rs:2152 |
| test508 | `test508_reorder_buffer_duplicate_buffered_seq` | TEST508: ReorderBuffer rejects duplicate seq already in buffer | src/bifaci/frame.rs:2180 |
| test509 | `test509_reorder_buffer_large_gap_rejected` | TEST509: ReorderBuffer handles large seq gaps without DOS | src/bifaci/frame.rs:2197 |
| test510 | `test510_reorder_buffer_multiple_gaps` | TEST510: ReorderBuffer with multiple interleaved gaps fills correctly | src/bifaci/frame.rs:2222 |
| test511 | `test511_reorder_buffer_cleanup_with_buffered_frames` | TEST511: ReorderBuffer cleanup with buffered frames discards them | src/bifaci/frame.rs:2255 |
| test512 | `test512_reorder_buffer_burst_delivery` | TEST512: ReorderBuffer delivers burst of consecutive buffered frames | src/bifaci/frame.rs:2278 |
| test513 | `test513_reorder_buffer_mixed_types_same_flow` | TEST513: ReorderBuffer different frame types in same flow maintain order | src/bifaci/frame.rs:2298 |
| test514 | `test514_reorder_buffer_xid_cleanup_isolation` | TEST514: ReorderBuffer with XID cleanup doesn't affect different XID | src/bifaci/frame.rs:2323 |
| test515 | `test515_reorder_buffer_overflow_error_details` | TEST515: ReorderBuffer overflow error includes diagnostic information | src/bifaci/frame.rs:2348 |
| test516 | `test516_reorder_buffer_stale_error_details` | TEST516: ReorderBuffer stale error includes diagnostic information | src/bifaci/frame.rs:2371 |
| test517 | `test517_flow_key_none_vs_some_xid` | TEST517: FlowKey with None XID differs from Some(xid) | src/bifaci/frame.rs:2391 |
| test518 | `test518_reorder_buffer_empty_ready_vec` | TEST518: ReorderBuffer handles zero-length ready vec correctly | src/bifaci/frame.rs:2417 |
| test519 | `test519_reorder_buffer_state_persistence` | TEST519: ReorderBuffer state persists across accept calls | src/bifaci/frame.rs:2429 |
| test520 | `test520_reorder_buffer_per_flow_limit` | TEST520: ReorderBuffer max_buffer_per_flow is per-flow not global | src/bifaci/frame.rs:2447 |
| test521 | `test521_relay_notify_cbor_roundtrip` | TEST521: RelayNotify CBOR roundtrip preserves manifest and limits | src/bifaci/frame.rs:2475 |
| test522 | `test522_relay_state_cbor_roundtrip` | TEST522: RelayState CBOR roundtrip preserves payload | src/bifaci/frame.rs:2501 |
| test523 | `test523_relay_notify_not_flow_frame` | TEST523: is_flow_frame returns false for RelayNotify | src/bifaci/frame.rs:2518 |
| test524 | `test524_relay_state_not_flow_frame` | TEST524: is_flow_frame returns false for RelayState | src/bifaci/frame.rs:2529 |
| test525 | `test525_relay_notify_empty_manifest` | TEST525: RelayNotify with empty manifest is valid | src/bifaci/frame.rs:2539 |
| test526 | `test526_relay_state_empty_payload` | TEST526: RelayState with empty payload is valid | src/bifaci/frame.rs:2550 |
| test527 | `test527_relay_notify_large_manifest` | TEST527: RelayNotify with large manifest roundtrips correctly | src/bifaci/frame.rs:2560 |
| test528 | `test528_relay_frames_use_uint_zero_id` | TEST528: RelayNotify and RelayState use MessageId::Uint(0) | src/bifaci/frame.rs:2587 |
| test529 | `test529_input_stream_recv_order` | TEST529: InputStream recv yields chunks in order | src/bifaci/plugin_runtime.rs:5592 |
| test530 | `test530_input_stream_collect_bytes` | TEST530: InputStream::collect_bytes concatenates byte chunks | src/bifaci/plugin_runtime.rs:5612 |
| test531 | `test531_input_stream_collect_bytes_text` | TEST531: InputStream::collect_bytes handles text chunks | src/bifaci/plugin_runtime.rs:5626 |
| test532 | `test532_input_stream_empty` | TEST532: InputStream empty stream produces empty bytes | src/bifaci/plugin_runtime.rs:5639 |
| test533 | `test533_input_stream_error_propagation` | TEST533: InputStream propagates errors | src/bifaci/plugin_runtime.rs:5649 |
| test534 | `test534_input_stream_media_urn` | TEST534: InputStream::media_urn returns correct URN | src/bifaci/plugin_runtime.rs:5668 |
| test535 | `test535_input_package_iteration` | TEST535: InputPackage recv yields streams | src/bifaci/plugin_runtime.rs:5677 |
| test536 | `test536_input_package_collect_all_bytes` | TEST536: InputPackage::collect_all_bytes aggregates all streams | src/bifaci/plugin_runtime.rs:5713 |
| test537 | `test537_input_package_empty` | TEST537: InputPackage empty package produces empty bytes | src/bifaci/plugin_runtime.rs:5747 |
| test538 | `test538_input_package_error_propagation` | TEST538: InputPackage propagates stream errors | src/bifaci/plugin_runtime.rs:5762 |
| test539 | `test539_output_stream_sends_stream_start` | TEST539: OutputStream sends STREAM_START on first write | src/bifaci/plugin_runtime.rs:5818 |
| test540 | `test540_output_stream_close_sends_stream_end` | TEST540: OutputStream::close sends STREAM_END with correct chunk_count | src/bifaci/plugin_runtime.rs:5840 |
| test541 | `test541_output_stream_chunks_large_data` | TEST541: OutputStream chunks large data correctly | src/bifaci/plugin_runtime.rs:5867 |
| test542 | `test542_output_stream_empty` | TEST542: OutputStream empty stream sends STREAM_START and STREAM_END only | src/bifaci/plugin_runtime.rs:5894 |
| test543 | `test543_peer_call_arg_creates_stream` | TEST543: PeerCall::arg creates OutputStream with correct stream_id | src/bifaci/plugin_runtime.rs:5919 |
| test544 | `test544_peer_call_finish_sends_end` | TEST544: PeerCall::finish sends END frame | src/bifaci/plugin_runtime.rs:5937 |
| test545 | `test545_peer_call_finish_returns_response_stream` | TEST545: PeerCall::finish returns InputStream for response | src/bifaci/plugin_runtime.rs:5972 |
| test546 | `test546_is_image` | TEST546: is_image returns true only when image marker tag is present | src/urn/media_urn.rs:886 |
| test547 | `test547_is_audio` | TEST547: is_audio returns true only when audio marker tag is present | src/urn/media_urn.rs:899 |
| test548 | `test548_is_video` | TEST548: is_video returns true only when video marker tag is present | src/urn/media_urn.rs:911 |
| test549 | `test549_is_numeric` | TEST549: is_numeric returns true only when numeric marker tag is present | src/urn/media_urn.rs:922 |
| test550 | `test550_is_bool` | TEST550: is_bool returns true only when bool marker tag is present | src/urn/media_urn.rs:935 |
| test551 | `test551_is_file_path` | TEST551: is_file_path returns true for scalar file-path, false for array | src/urn/media_urn.rs:948 |
| test552 | `test552_is_file_path_array` | TEST552: is_file_path_array returns true for list file-path, false for scalar | src/urn/media_urn.rs:959 |
| test553 | `test553_is_any_file_path` | TEST553: is_any_file_path returns true for both scalar and array file-path | src/urn/media_urn.rs:969 |
| test555 | `test555_with_tag_and_without_tag` | TEST555: with_tag adds a tag and without_tag removes it | src/urn/media_urn.rs:979 |
| test556 | `test556_image_media_urn_for_ext` | TEST556: image_media_urn_for_ext creates valid image media URN | src/urn/media_urn.rs:996 |
| test557 | `test557_audio_media_urn_for_ext` | TEST557: audio_media_urn_for_ext creates valid audio media URN | src/urn/media_urn.rs:1006 |
| test558 | `test558_predicate_constant_consistency` | TEST558: predicates are consistent with constants — every constant triggers exactly the expected predicates | src/urn/media_urn.rs:1016 |
| test559 | `test559_without_tag` | TEST559: without_tag removes tag, ignores in/out, case-insensitive for keys | src/urn/cap_urn.rs:2020 |
| test560 | `test560_with_in_out_spec` | TEST560: with_in_spec and with_out_spec change direction specs | src/urn/cap_urn.rs:2045 |
| test561 | `test561_in_out_media_urn` | TEST561: in_media_urn and out_media_urn parse direction specs into MediaUrn | src/urn/cap_urn.rs:2069 |
| test562 | `test562_canonical_option` | TEST562: canonical_option returns None for None input, canonical string for Some | src/urn/cap_urn.rs:2091 |
| test563 | `test563_find_all_matches` | TEST563: CapMatcher::find_all_matches returns all matching caps sorted by specificity | src/urn/cap_urn.rs:2113 |
| test564 | `test564_are_compatible` | TEST564: CapMatcher::are_compatible detects bidirectional overlap | src/urn/cap_urn.rs:2132 |
| test565 | `test565_tags_to_string` | TEST565: tags_to_string returns only tags portion without prefix | src/urn/cap_urn.rs:2156 |
| test566 | `test566_with_tag_ignores_in_out` | TEST566: with_tag silently ignores in/out keys | src/urn/cap_urn.rs:2169 |
| test567 | `test567_str_variants` | TEST567: conforms_to_str and accepts_str work with string arguments | src/urn/cap_urn.rs:2183 |
| test568 | `test568_dispatch_output_tag_order` | TEST568: is_dispatchable with different tag order in output spec | src/urn/cap_urn.rs:2202 |
| test569 | `test569_unregister_cap_set` | TEST569: unregister_cap_set removes a host and returns true, false if not found | src/urn/cap_matrix.rs:1855 |
| test570 | `test570_clear` | TEST570: clear removes all registered sets | src/urn/cap_matrix.rs:1875 |
| test571 | `test571_get_all_capabilities` | TEST571: get_all_capabilities returns caps from all hosts | src/urn/cap_matrix.rs:1892 |
| test572 | `test572_get_capabilities_for_host` | TEST572: get_capabilities_for_host returns caps for specific host, None for unknown | src/urn/cap_matrix.rs:1910 |
| test573 | `test573_iter_hosts_and_caps` | TEST573: iter_hosts_and_caps iterates all hosts with their capabilities | src/urn/cap_matrix.rs:1927 |
| test574 | `test574_cap_block_remove_registry` | TEST574: CapBlock::remove_registry removes by name, returns Arc | src/urn/cap_matrix.rs:1946 |
| test575 | `test575_cap_block_get_registry` | TEST575: CapBlock::get_registry returns Arc clone by name | src/urn/cap_matrix.rs:1967 |
| test576 | `test576_cap_block_get_registry_names` | TEST576: CapBlock::get_registry_names returns names in insertion order | src/urn/cap_matrix.rs:1983 |
| test577 | `test577_cap_graph_input_output_specs` | TEST577: CapGraph::get_input_specs and get_output_specs return correct sets | src/urn/cap_matrix.rs:1998 |
| test578 | `test578_rule1_duplicate_media_urns` | TEST578: RULE1 - duplicate media_urns rejected | src/cap/validation.rs:1246 |
| test579 | `test579_rule2_empty_sources` | TEST579: RULE2 - empty sources rejected | src/cap/validation.rs:1259 |
| test580 | `test580_rule3_different_stdin_urns` | TEST580: RULE3 - multiple stdin sources with different URNs rejected | src/cap/validation.rs:1271 |
| test581 | `test581_rule3_same_stdin_urns_ok` | TEST581: RULE3 - multiple stdin sources with same URN is OK | src/cap/validation.rs:1284 |
| test582 | `test582_rule4_duplicate_source_type` | TEST582: RULE4 - duplicate source type in single arg rejected | src/cap/validation.rs:1295 |
| test583 | `test583_rule5_duplicate_position` | TEST583: RULE5 - duplicate position across args rejected | src/cap/validation.rs:1310 |
| test584 | `test584_rule6_position_gap` | TEST584: RULE6 - position gap rejected (0, 2 without 1) | src/cap/validation.rs:1323 |
| test585 | `test585_rule6_sequential_ok` | TEST585: RULE6 - sequential positions (0, 1, 2) pass | src/cap/validation.rs:1336 |
| test586 | `test586_rule7_position_and_cli_flag` | TEST586: RULE7 - arg with both position and cli_flag rejected | src/cap/validation.rs:1347 |
| test587 | `test587_rule9_duplicate_cli_flag` | TEST587: RULE9 - duplicate cli_flag across args rejected | src/cap/validation.rs:1362 |
| test588 | `test588_rule10_reserved_cli_flags` | TEST588: RULE10 - reserved cli_flags rejected | src/cap/validation.rs:1375 |
| test589 | `test589_all_rules_pass` | TEST589: valid cap args with mixed sources pass all rules | src/cap/validation.rs:1389 |
| test590 | `test590_cli_flag_only_args` | TEST590: validate_cap_args accepts cap with only cli_flag sources (no positions) | src/cap/validation.rs:1405 |
| test591 | `test591_is_more_specific_than` | TEST591: is_more_specific_than returns true when self has more tags for same request | src/cap/definition.rs:1047 |
| test592 | `test592_remove_metadata` | TEST592: remove_metadata adds then removes metadata correctly | src/cap/definition.rs:1083 |
| test593 | `test593_registered_by_lifecycle` | TEST593: registered_by lifecycle — set, get, clear | src/cap/definition.rs:1103 |
| test594 | `test594_metadata_json_lifecycle` | TEST594: metadata_json lifecycle — set, get, clear | src/cap/definition.rs:1124 |
| test595 | `test595_with_args_constructor` | TEST595: with_args constructor stores args correctly | src/cap/definition.rs:1143 |
| test596 | `test596_with_full_definition_constructor` | TEST596: with_full_definition constructor stores all fields | src/cap/definition.rs:1160 |
| test597 | `test597_cap_arg_with_full_definition` | TEST597: CapArg::with_full_definition stores all fields including optional ones | src/cap/definition.rs:1188 |
| test598 | `test598_cap_output_lifecycle` | TEST598: CapOutput lifecycle — set_output, set/clear metadata | src/cap/definition.rs:1216 |
| test599 | `test599_is_empty` | TEST599: is_empty returns true for empty response, false for non-empty | src/cap/response.rs:297 |
| test600 | `test600_size` | TEST600: size returns exact byte count for all content types | src/cap/response.rs:313 |
| test601 | `test601_get_content_type` | TEST601: get_content_type returns correct MIME type for each variant | src/cap/response.rs:329 |
| test602 | `test602_as_type_binary_error` | TEST602: as_type on binary response returns error (cannot deserialize binary) | src/cap/response.rs:342 |
| test603 | `test603_as_bool_edge_cases` | TEST603: as_bool handles all accepted truthy/falsy variants and rejects garbage | src/cap/response.rs:352 |
| test605 | `test605_all_coercion_paths_build_valid_urns` | TEST605: all_coercion_paths each entry builds a valid parseable CapUrn | src/standard/caps.rs:827 |
| test606 | `test606_coercion_urn_specs` | TEST606: coercion_urn in/out specs match the type's media URN constant | src/standard/caps.rs:849 |
| test607 | `test607_media_urns_for_extension_unknown` | TEST607: media_urns_for_extension returns error for unknown extension | src/media/registry.rs:764 |
| test608 | `test608_media_urns_for_extension_populated` | TEST608: media_urns_for_extension returns URNs after adding a spec with extensions | src/media/registry.rs:778 |
| test609 | `test609_get_extension_mappings` | TEST609: get_extension_mappings returns all registered extension->URN pairs | src/media/registry.rs:812 |
| test610 | `test610_get_cached_spec` | TEST610: get_cached_spec returns None for unknown and Some for known | src/media/registry.rs:837 |
| test611 | `test611_is_embedded_profile_comprehensive` | TEST611: is_embedded_profile recognizes all 9 embedded profiles and rejects non-embedded | src/media/profile.rs:650 |
| test612 | `test612_clear_cache` | TEST612: clear_cache empties all in-memory schemas | src/media/profile.rs:671 |
| test613 | `test613_validate_cached` | TEST613: validate_cached validates against cached standard schemas | src/media/profile.rs:688 |
| test614 | `test614_registry_creation` | TEST614: Verify registry creation succeeds and cache directory exists | src/media/registry.rs:710 |
| test615 | `test615_cache_key_generation` | TEST615: Verify cache key generation is deterministic and distinct for different URNs | src/media/registry.rs:717 |
| test616 | `test616_stored_media_spec_to_def` | TEST616: Verify StoredMediaSpec converts to MediaSpecDef preserving all fields | src/media/registry.rs:729 |
| test617 | `test617_normalize_media_urn` | TEST617: Verify normalize_media_urn produces consistent non-empty results | src/media/registry.rs:753 |
| test618 | `test618_registry_creation` | TEST618: Verify profile schema registry creation succeeds with temp cache | src/media/profile.rs:526 |
| test619 | `test619_embedded_schemas_loaded` | TEST619: Verify all 9 embedded standard schemas are loaded on creation | src/media/profile.rs:533 |
| test620 | `test620_string_validation` | TEST620: Verify string schema validates strings and rejects non-strings | src/media/profile.rs:550 |
| test621 | `test621_integer_validation` | TEST621: Verify integer schema validates integers and rejects floats and strings | src/media/profile.rs:562 |
| test622 | `test622_number_validation` | TEST622: Verify number schema validates integers and floats, rejects strings | src/media/profile.rs:577 |
| test623 | `test623_boolean_validation` | TEST623: Verify boolean schema validates true/false and rejects string "true" | src/media/profile.rs:592 |
| test624 | `test624_object_validation` | TEST624: Verify object schema validates objects and rejects arrays | src/media/profile.rs:605 |
| test625 | `test625_string_array_validation` | TEST625: Verify string array schema validates string arrays and rejects mixed arrays | src/media/profile.rs:617 |
| test626 | `test626_unknown_profile_skips_validation` | TEST626: Verify unknown profile URL skips validation and returns Ok | src/media/profile.rs:632 |
| test627 | `test627_is_embedded_profile` | TEST627: Verify is_embedded_profile recognizes standard and rejects custom URLs | src/media/profile.rs:642 |
| test628 | `test628_media_urn_constants_format` | TEST628: Verify media URN constants all start with "media:" prefix | src/standard/media.rs:68 |
| test629 | `test629_profile_constants_format` | TEST629: Verify profile URL constants all start with capdag.com schema prefix | src/standard/media.rs:78 |
| test630 | `test630_plugin_repo_creation` | TEST630: Verify PluginRepo creation starts with empty plugin list | src/bifaci/plugin_repo.rs:592 |
| test631 | `test631_needs_sync_empty_cache` | TEST631: Verify needs_sync returns true with empty cache and non-empty URLs | src/bifaci/plugin_repo.rs:599 |
| test632 | `test632_deserialize_cap_summary_with_null_description` | TEST632: Verify PluginCapSummary deserializes null description as empty string | src/bifaci/plugin_repo.rs:607 |
| test633 | `test633_deserialize_cap_summary_with_missing_description` | TEST633: Verify PluginCapSummary deserializes missing description as empty string | src/bifaci/plugin_repo.rs:617 |
| test634 | `test634_deserialize_cap_summary_with_present_description` | TEST634: Verify PluginCapSummary deserializes present description correctly | src/bifaci/plugin_repo.rs:625 |
| test635 | `test635_deserialize_plugin_info_with_null_fields` | TEST635: Verify PluginInfo deserializes null version/description/author as empty strings | src/bifaci/plugin_repo.rs:633 |
| test636 | `test636_deserialize_registry_with_null_descriptions` | TEST636: Verify PluginRegistryResponse deserializes with mixed null/present descriptions | src/bifaci/plugin_repo.rs:656 |
| test637 | `test637_deserialize_full_plugin_with_signature` | TEST637: Verify full PluginInfo deserialization with signature and binary fields | src/bifaci/plugin_repo.rs:678 |
| test638 | `test638_no_peer_router_rejects_all` | TEST638: Verify NoPeerRouter rejects all requests with PeerInvokeNotSupported | src/bifaci/router.rs:95 |
| test639 | `test639_wildcard_001_empty_cap_defaults_to_media_wildcard` | TEST639: cap: (empty) defaults to in=media:;out=media: | src/urn/cap_urn.rs:1832 |
| test640 | `test640_wildcard_002_in_only_defaults_out_to_media` | TEST640: cap:in defaults out to media: | src/urn/cap_urn.rs:1841 |
| test641 | `test641_wildcard_003_out_only_defaults_in_to_media` | TEST641: cap:out defaults in to media: | src/urn/cap_urn.rs:1849 |
| test642 | `test642_wildcard_004_in_out_no_values_become_media` | TEST642: cap:in;out both become media: | src/urn/cap_urn.rs:1857 |
| test643 | `test643_wildcard_005_explicit_asterisk_becomes_media` | TEST643: cap:in=*;out=* becomes media: | src/urn/cap_urn.rs:1865 |
| test644 | `test644_wildcard_006_specific_in_wildcard_out` | TEST644: cap:in=media:;out=* has specific in, wildcard out | src/urn/cap_urn.rs:1873 |
| test645 | `test645_wildcard_007_wildcard_in_specific_out` | TEST645: cap:in=*;out=media:text has wildcard in, specific out | src/urn/cap_urn.rs:1881 |
| test646 | `test646_wildcard_008_invalid_in_spec_fails` | TEST646: cap:in=foo fails (invalid media URN) | src/urn/cap_urn.rs:1889 |
| test647 | `test647_wildcard_009_invalid_out_spec_fails` | TEST647: cap:in=media:;out=bar fails (invalid media URN) | src/urn/cap_urn.rs:1898 |
| test648 | `test648_wildcard_010_wildcard_accepts_specific` | TEST648: Wildcard in/out match specific caps | src/urn/cap_urn.rs:1907 |
| test649 | `test649_wildcard_011_specificity_scoring` | TEST649: Specificity - wildcard has 0, specific has tag count | src/urn/cap_urn.rs:1917 |
| test650 | `test650_wildcard_012_preserve_other_tags` | TEST650: cap:in;out;op=test preserves other tags | src/urn/cap_urn.rs:1927 |
| test651 | `test651_wildcard_013_identity_forms_equivalent` | TEST651: All identity forms produce the same CapUrn | src/urn/cap_urn.rs:1936 |
| test652 | `test652_wildcard_014_cap_identity_constant_works` | TEST652: CAP_IDENTITY constant matches identity caps regardless of string form | src/urn/cap_urn.rs:1961 |
| test653 | `test653_wildcard_015_identity_routing_isolation` | TEST653: Identity (no tags) does not match specific requests via routing | src/urn/cap_urn.rs:1991 |
| test654 | `test654_routes_req_to_handler` | TEST654: InProcessPluginHost routes REQ to matching handler and returns response | src/bifaci/in_process_host.rs:633 |
| test655 | `test655_identity_verification` | TEST655: InProcessPluginHost handles identity verification (echo nonce) | src/bifaci/in_process_host.rs:711 |
| test656 | `test656_no_handler_returns_err` | TEST656: InProcessPluginHost returns NO_HANDLER for unregistered cap | src/bifaci/in_process_host.rs:781 |
| test657 | `test657_manifest_includes_all_caps` | TEST657: InProcessPluginHost manifest includes identity cap and handler caps | src/bifaci/in_process_host.rs:821 |
| test658 | `test658_heartbeat_response` | TEST658: InProcessPluginHost handles heartbeat by echoing same ID | src/bifaci/in_process_host.rs:838 |
| test659 | `test659_handler_error_returns_err_frame` | TEST659: InProcessPluginHost handler error returns ERR frame | src/bifaci/in_process_host.rs:870 |
| test660 | `test660_closest_specificity_routing` | TEST660: InProcessPluginHost closest-specificity routing prefers specific over identity | src/bifaci/in_process_host.rs:941 |
| test661 | `test661_plugin_death_keeps_known_caps_advertised` | TEST661: Plugin death keeps known_caps advertised for on-demand respawn | src/bifaci/host_runtime.rs:2767 |
| test662 | `test662_rebuild_capabilities_includes_non_running_plugins` | TEST662: rebuild_capabilities includes non-running plugins' known_caps | src/bifaci/host_runtime.rs:2798 |
| test663 | `test663_hello_failed_plugin_removed_from_capabilities` | TEST663: Plugin with hello_failed is permanently removed from capabilities | src/bifaci/host_runtime.rs:2831 |
| test664 | `test664_running_plugin_uses_manifest_caps` | TEST664: Running plugin uses manifest caps, not known_caps | src/bifaci/host_runtime.rs:2867 |
| test665 | `test665_cap_table_mixed_running_and_non_running` | TEST665: Cap table uses manifest caps for running, known_caps for non-running | src/bifaci/host_runtime.rs:2920 |
| test666 | `test666_preferred_cap_routing` | TEST666: Preferred cap routing - routes to exact equivalent when multiple masters match | src/bifaci/relay_switch.rs:2620 |
| test667 | `test667_verify_chunk_checksum_detects_corruption` | TEST667: verify_chunk_checksum detects corrupted payload | src/bifaci/frame.rs:2603 |
| test668 | `test668_resolve_slot_with_populated_byte_slot_values` |  | src/planner/argument_binding.rs:723 |
| test669 | `test669_resolve_slot_falls_back_to_default` |  | src/planner/argument_binding.rs:754 |
| test670 | `test670_resolve_required_slot_no_value_returns_err` |  | src/planner/argument_binding.rs:777 |
| test671 | `test671_resolve_optional_slot_no_value_returns_none` |  | src/planner/argument_binding.rs:799 |
| test675 | `test675_build_request_frames_preserves_media_urn_in_stream_start` | TEST675: build_request_frames with full media URN preserves it in STREAM_START frame | src/cap/caller.rs:545 |
| test676 | `test676_build_request_frames_round_trip_find_stream_succeeds` | TEST676: Full round-trip: build_request_frames → extract streams → find_stream succeeds | src/cap/caller.rs:568 |
| test677 | `test677_base_urn_does_not_match_full_urn_in_find_stream` | TEST677: build_request_frames with BASE URN → find_stream with FULL URN FAILS This documents the root cause of the cartridge_client.rs bug: sender used "media:llm-generation-request" (base), receiver looked for "media:llm-generation-request;json;record" (full). is_equivalent requires exact tag set match, so base != full. | src/cap/caller.rs:621 |
| test678 | `test678_find_stream_equivalent_urn_different_tag_order` | TEST678: find_stream with exact equivalent URN (same tags, different order) succeeds | src/bifaci/plugin_runtime.rs:6030 |
| test679 | `test679_find_stream_base_urn_does_not_match_full_urn` | TEST679: find_stream with base URN vs full URN fails — is_equivalent is strict This is the root cause of the cartridge_client.rs bug. Sender sent "media:llm-generation-request" but receiver looked for "media:llm-generation-request;json;record". | src/bifaci/plugin_runtime.rs:6045 |
| test680 | `test680_require_stream_missing_urn_returns_error` | TEST680: require_stream with missing URN returns hard StreamError | src/bifaci/plugin_runtime.rs:6058 |
| test681 | `test681_find_stream_multiple_streams_returns_correct` | TEST681: find_stream with multiple streams returns the correct one | src/bifaci/plugin_runtime.rs:6074 |
| test682 | `test682_require_stream_str_returns_utf8` | TEST682: require_stream_str returns UTF-8 string for text data | src/bifaci/plugin_runtime.rs:6087 |
| test683 | `test683_find_stream_invalid_urn_returns_none` | TEST683: find_stream returns None for invalid media URN string (not a parse error — just None) | src/bifaci/plugin_runtime.rs:6097 |
| test684 | `test684_from_media_urn_single` | TEST684: Tests InputCardinality correctly identifies single-value media URNs Verifies that URNs without list marker are parsed as Single cardinality | src/planner/cardinality.rs:540 |
| test685 | `test685_from_media_urn_vector` | TEST685: Tests InputCardinality correctly identifies list/vector media URNs Verifies that URNs with list marker tag are parsed as Sequence cardinality | src/planner/cardinality.rs:551 |
| test686 | `test686_from_media_urn_vector_tag_position` | TEST686: Tests that list marker tag position doesn't affect vector detection Verifies cardinality parsing is independent of tag order in URN | src/planner/cardinality.rs:562 |
| test687 | `test687_from_media_urn_no_false_positives` | TEST687: Tests that URN content doesn't cause false positive vector detection Verifies that "list" in media type name doesn't trigger Sequence cardinality | src/planner/cardinality.rs:570 |
| test688 | `test688_is_multiple` | TEST688: Tests is_multiple method correctly identifies multi-value cardinalities Verifies Single returns false while Sequence and AtLeastOne return true | src/planner/cardinality.rs:579 |
| test689 | `test689_accepts_single` | TEST689: Tests accepts_single method identifies cardinalities that accept single values Verifies Single and AtLeastOne accept singles while Sequence does not | src/planner/cardinality.rs:588 |
| test690 | `test690_compatibility_single_to_single` | TEST690: Tests cardinality compatibility for single-to-single data flow Verifies Direct compatibility when both input and output are Single | src/planner/cardinality.rs:599 |
| test691 | `test691_compatibility_single_to_vector` | TEST691: Tests cardinality compatibility when wrapping single value into array Verifies WrapInArray compatibility when Sequence expects Single input | src/planner/cardinality.rs:606 |
| test692 | `test692_compatibility_vector_to_single` | TEST692: Tests cardinality compatibility when unwrapping array to singles Verifies RequiresFanOut compatibility when Single expects Sequence input | src/planner/cardinality.rs:613 |
| test693 | `test693_compatibility_vector_to_vector` | TEST693: Tests cardinality compatibility for sequence-to-sequence data flow Verifies Direct compatibility when both input and output are Sequence | src/planner/cardinality.rs:620 |
| test694 | `test694_apply_to_urn_add_vector` | TEST694: Tests applying Sequence cardinality adds list marker to URN Verifies that apply_to_urn correctly modifies URN to indicate list | src/planner/cardinality.rs:629 |
| test695 | `test695_apply_to_urn_remove_vector` | TEST695: Tests applying Single cardinality removes list marker from URN Verifies that apply_to_urn correctly strips list marker | src/planner/cardinality.rs:638 |
| test696 | `test696_apply_to_urn_no_change_needed` | TEST696: Tests apply_to_urn is idempotent when URN already matches cardinality Verifies that URN remains unchanged when cardinality already matches desired | src/planner/cardinality.rs:646 |
| test697 | `test697_cap_shape_info_one_to_one` | TEST697: Tests CapShapeInfo correctly identifies one-to-one pattern Verifies Single input and Single output result in OneToOne pattern | src/planner/cardinality.rs:658 |
| test698 | `test698_cap_shape_info_one_to_many` | TEST698: Tests CapShapeInfo correctly identifies one-to-many pattern Verifies Single input and Sequence output result in OneToMany pattern | src/planner/cardinality.rs:668 |
| test699 | `test699_cap_shape_info_many_to_one` | TEST699: Tests CapShapeInfo correctly identifies many-to-one pattern Verifies Sequence input and Single output result in ManyToOne pattern | src/planner/cardinality.rs:678 |
| test700 | `test700_filepath_conversion_scalar` | TEST700: File-path conversion with test-edge1 (scalar file input) | testcartridge/tests/integration_tests.rs:18 |
| test701 | `test701_filepath_array_glob` | TEST701: File-path array with glob expansion (test-edge3) | testcartridge/tests/integration_tests.rs:46 |
| test702 | `test702_large_payload_1mb` | TEST702: Large payload auto-chunking (1MB response) | testcartridge/tests/integration_tests.rs:72 |
| test703 | `test703_peer_invoke_chain` |  | testcartridge/tests/integration_tests.rs:96 |
| test704 | `test704_multi_argument` | TEST704: Multi-argument cap (test-edge5) | testcartridge/tests/integration_tests.rs:103 |
| test705 | `test705_piped_stdin` | TEST705: Piped stdin input (no file-path conversion) | testcartridge/tests/integration_tests.rs:132 |
| test706 | `test706_empty_file` | TEST706: Empty file handling | testcartridge/tests/integration_tests.rs:155 |
| test707 | `test707_utf8_file` | TEST707: UTF-8 file handling (textable constraint) | testcartridge/tests/integration_tests.rs:177 |
| test708 | `test708_missing_file` | TEST708: Missing file error handling | testcartridge/tests/integration_tests.rs:200 |
| test709 | `test709_pattern_produces_vector` | TEST709: Tests CardinalityPattern correctly identifies patterns that produce vectors Verifies OneToMany and ManyToMany return true, others return false | src/planner/cardinality.rs:690 |
| test710 | `test710_pattern_requires_vector` | TEST710: Tests CardinalityPattern correctly identifies patterns that require vectors Verifies ManyToOne and ManyToMany return true, others return false | src/planner/cardinality.rs:700 |
| test711 | `test711_shape_chain_analysis_simple_linear` | TEST711: Tests shape chain analysis for simple linear one-to-one capability chains Verifies chains with no fan-out are valid and require no transformation | src/planner/cardinality.rs:712 |
| test712 | `test712_shape_chain_analysis_with_fan_out` | TEST712: Tests shape chain analysis detects fan-out points in capability chains Verifies chains with one-to-many transitions are marked for transformation | src/planner/cardinality.rs:726 |
| test713 | `test713_shape_chain_analysis_empty` | TEST713: Tests shape chain analysis handles empty capability chains correctly Verifies empty chains are valid and require no transformation | src/planner/cardinality.rs:740 |
| test714 | `test714_cardinality_serialization` | TEST714: Tests InputCardinality serializes and deserializes correctly to/from JSON Verifies JSON round-trip preserves cardinality values | src/planner/cardinality.rs:751 |
| test715 | `test715_pattern_serialization` | TEST715: Tests CardinalityPattern serializes and deserializes correctly to/from JSON Verifies JSON round-trip preserves pattern values with snake_case formatting | src/planner/cardinality.rs:762 |
| test716 | `test716_empty_collection` | TEST716: Tests CapInputCollection empty collection has zero files and folders Verifies is_empty() returns true and counts are zero for new collection | src/planner/collection_input.rs:161 |
| test717 | `test717_collection_with_files` | TEST717: Tests CapInputCollection correctly counts files in flat collection Verifies total_file_count() returns 2 for collection with 2 files, no folders | src/planner/collection_input.rs:174 |
| test718 | `test718_nested_collection` | TEST718: Tests CapInputCollection correctly counts files and folders in nested structure Verifies total_file_count() includes subfolder files and total_folder_count() counts subfolders | src/planner/collection_input.rs:198 |
| test719 | `test719_flatten_to_files` | TEST719: Tests CapInputCollection flatten_to_files recursively collects all files Verifies flatten() extracts files from root and all subfolders into flat list | src/planner/collection_input.rs:233 |
| test720 | `test720_from_media_urn_opaque` | TEST720: Tests InputStructure correctly identifies opaque media URNs Verifies that URNs without record marker are parsed as Opaque | src/planner/cardinality.rs:775 |
| test721 | `test721_from_media_urn_record` | TEST721: Tests InputStructure correctly identifies record media URNs Verifies that URNs with record marker tag are parsed as Record | src/planner/cardinality.rs:786 |
| test722 | `test722_structure_compatibility_opaque_to_opaque` | TEST722: Tests structure compatibility for opaque-to-opaque data flow | src/planner/cardinality.rs:796 |
| test723 | `test723_structure_compatibility_record_to_record` | TEST723: Tests structure compatibility for record-to-record data flow | src/planner/cardinality.rs:805 |
| test724 | `test724_structure_incompatibility_opaque_to_record` | TEST724: Tests structure incompatibility for opaque-to-record flow | src/planner/cardinality.rs:814 |
| test725 | `test725_structure_incompatibility_record_to_opaque` | TEST725: Tests structure incompatibility for record-to-opaque flow | src/planner/cardinality.rs:822 |
| test726 | `test726_apply_structure_add_record` | TEST726: Tests applying Record structure adds record marker to URN | src/planner/cardinality.rs:830 |
| test727 | `test727_apply_structure_remove_record` | TEST727: Tests applying Opaque structure removes record marker from URN | src/planner/cardinality.rs:837 |
| test728 | `test728_cap_node_helpers` | TEST728: Tests CapNode helper methods for identifying node types (cap, fan-out, fan-in) Verifies is_cap(), is_fan_out(), is_fan_in(), and cap_urn() correctly classify node types | src/planner/plan.rs:1062 |
| test729 | `test729_edge_types` | TEST729: Tests creation and classification of different edge types (Direct, Iteration, Collection, JsonField) Verifies that edge constructors produce correct EdgeType variants | src/planner/plan.rs:1084 |
| test730 | `test730_media_shape_from_urn_all_combinations` | TEST730: Tests MediaShape correctly parses all four combinations | src/planner/cardinality.rs:846 |
| test731 | `test731_media_shape_compatible_direct` | TEST731: Tests MediaShape compatibility for matching shapes | src/planner/cardinality.rs:870 |
| test732 | `test732_media_shape_cardinality_changes` | TEST732: Tests MediaShape compatibility for cardinality changes with matching structure | src/planner/cardinality.rs:885 |
| test733 | `test733_media_shape_structure_mismatch` | TEST733: Tests MediaShape incompatibility when structures don't match | src/planner/cardinality.rs:902 |
| test734 | `test734_topological_order_self_loop` | TEST734: Tests topological sort detects self-referencing cycles (A→A) Verifies that self-loops are recognized as cycles and produce an error | src/planner/plan.rs:1173 |
| test735 | `test735_topological_order_multiple_entry_points` | TEST735: Tests topological sort handles graphs with multiple independent starting nodes Verifies that parallel entry points (A→C, B→C) both precede their merge point in ordering | src/planner/plan.rs:1189 |
| test736 | `test736_topological_order_complex_dag` | TEST736: Tests topological sort on a complex multi-path DAG with 6 nodes Verifies that all dependency constraints are satisfied in a graph with multiple converging paths | src/planner/plan.rs:1219 |
| test737 | `test737_linear_chain_single_cap` | TEST737: Tests linear_chain() with exactly one capability Verifies that a single-element chain produces a valid plan with input_slot, cap, and output | src/planner/plan.rs:1265 |
| test738 | `test738_linear_chain_empty` | TEST738: Tests linear_chain() with empty capability list Verifies that an empty chain produces a plan with zero nodes and edges | src/planner/plan.rs:1280 |
| test739 | `test739_node_execution_result_success` | TEST739: Tests NodeExecutionResult structure for successful node execution Verifies that success status, outputs (binary and text), and error fields work correctly | src/planner/plan.rs:1294 |
| test740 | `test740_cap_shape_info_from_specs` | TEST740: Tests CapShapeInfo correctly parses cap specs | src/planner/cardinality.rs:923 |
| test741 | `test741_cap_shape_info_pattern` | TEST741: Tests CapShapeInfo pattern detection | src/planner/cardinality.rs:937 |
| test742 | `test742_edge_type_serialization` | TEST742: Tests EdgeType enum serialization and deserialization to/from JSON Verifies that edge types like Direct and JsonField correctly round-trip through serde_json | src/planner/plan.rs:1347 |
| test743 | `test743_execution_node_type_serialization` | TEST743: Tests ExecutionNodeType enum serialization and deserialization to/from JSON Verifies that node types like Cap and ForEach correctly serialize with their fields | src/planner/plan.rs:1364 |
| test744 | `test744_plan_serialization` | TEST744: Tests CapExecutionPlan serialization and deserialization to/from JSON Verifies that complete plans with nodes and edges correctly round-trip through JSON | src/planner/plan.rs:1386 |
| test745 | `test745_merge_strategy_serialization` | TEST745: Tests MergeStrategy enum serialization to JSON Verifies that merge strategies like Concat and ZipWith serialize to correct string values | src/planner/plan.rs:1407 |
| test746 | `test746_cap_node_output` | TEST746: Tests creation of Output node type that references a source node Verifies that CapNode::output() correctly constructs an Output node with name and source | src/planner/plan.rs:1420 |
| test747 | `test747_cap_node_merge` | TEST747: Tests creation and validation of Merge node that combines multiple inputs Verifies that Merge nodes with multiple input nodes and a strategy can be added to plans | src/planner/plan.rs:1434 |
| test748 | `test748_cap_node_split` | TEST748: Tests creation of Split node that distributes input to multiple outputs Verifies that Split nodes correctly specify an input node and output count | src/planner/plan.rs:1459 |
| test749 | `test749_get_node` | TEST749: Tests get_node() method for looking up nodes by ID in a plan Verifies that existing nodes are found and non-existent nodes return None | src/planner/plan.rs:1481 |
| test750 | `test750_shape_chain_valid` | TEST750: Tests shape chain analysis for valid chain with matching structures | src/planner/cardinality.rs:950 |
| test751 | `test751_shape_chain_structure_mismatch` | TEST751: Tests shape chain analysis detects structure mismatch | src/planner/cardinality.rs:962 |
| test752 | `test752_shape_chain_with_fanout` | TEST752: Tests shape chain analysis with fan-out (matching structures) | src/planner/cardinality.rs:976 |
| test753 | `test753_shape_chain_list_record_to_list_record` | TEST753: Tests shape chain analysis correctly handles list-to-list record flow | src/planner/cardinality.rs:989 |
| test754 | `test754_extract_prefix_nonexistent` | TEST754: extract_prefix_to with nonexistent node returns error | src/planner/plan.rs:1599 |
| test755 | `test755_extract_foreach_body` | TEST755: extract_foreach_body extracts body as standalone plan | src/planner/plan.rs:1607 |
| test756 | `test756_extract_foreach_body_unclosed` | TEST756: extract_foreach_body for unclosed ForEach (single body cap) | src/planner/plan.rs:1643 |
| test757 | `test757_extract_foreach_body_wrong_type` | TEST757: extract_foreach_body fails for non-ForEach node | src/planner/plan.rs:1659 |
| test758 | `test758_extract_suffix_from` | TEST758: extract_suffix_from extracts collect → cap_post → output | src/planner/plan.rs:1669 |
| test759 | `test759_extract_suffix_nonexistent` | TEST759: extract_suffix_from fails for nonexistent node | src/planner/plan.rs:1689 |
| test760 | `test760_decomposition_covers_all_caps` | TEST760: Full decomposition roundtrip — prefix + body + suffix cover all cap nodes | src/planner/plan.rs:1697 |
| test761 | `test761_prefix_is_dag` | TEST761: Prefix sub-plan can be topologically sorted (is a valid DAG) | src/planner/plan.rs:1736 |
| test762 | `test762_body_is_dag` | TEST762: Body sub-plan can be topologically sorted (is a valid DAG) | src/planner/plan.rs:1744 |
| test763 | `test763_suffix_is_dag` | TEST763: Suffix sub-plan can be topologically sorted (is a valid DAG) | src/planner/plan.rs:1752 |
| test764 | `test764_extract_prefix_to_input_slot` | TEST764: extract_prefix_to with InputSlot as target (trivial prefix) | src/planner/plan.rs:1760 |
| test765 | `test765_validation_to_json_empty` | TEST765: Tests validation_to_json() returns None for empty validation constraints Verifies that default MediaValidation with no constraints produces JSON None | src/planner/plan_builder.rs:992 |
| test766 | `test766_validation_to_json_with_constraints` | TEST766: Tests validation_to_json() converts MediaValidation with constraints to JSON Verifies that min/max validation rules are correctly serialized as JSON fields | src/planner/plan_builder.rs:1001 |
| test767 | `test767_argument_info_serialization` | TEST767: Tests ArgumentInfo struct serialization to JSON Verifies that argument metadata including resolution status and validation is correctly serialized | src/planner/plan_builder.rs:1020 |
| test768 | `test768_path_argument_requirements_structure` | TEST768: Tests PathArgumentRequirements structure for single-step execution paths Verifies that argument requirements are correctly organized by step with resolution information | src/planner/plan_builder.rs:1040 |
| test769 | `test769_path_with_required_slot` | TEST769: Tests PathArgumentRequirements tracking of required user-input slots Verifies that arguments requiring user input are collected in slots and can_execute_without_input is false | src/planner/plan_builder.rs:1075 |
| test770 | `test770_rejects_foreach` | TEST770: plan_to_resolved_graph rejects plans containing ForEach nodes | src/orchestrator/plan_converter.rs:240 |
| test771 | `test771_rejects_collect` | TEST771: plan_to_resolved_graph rejects plans containing Collect nodes | src/orchestrator/plan_converter.rs:266 |
| test772 | `test772_find_paths_finds_multi_step_paths` | TEST772: Tests find_paths_to_exact_target() finds multi-step paths Verifies that paths through intermediate nodes are found correctly | src/planner/live_cap_graph.rs:1133 |
| test773 | `test773_find_paths_returns_empty_when_no_path` | TEST773: Tests find_paths_to_exact_target() returns empty when no path exists Verifies that pathfinding returns no paths when target is unreachable | src/planner/live_cap_graph.rs:1156 |
| test774 | `test774_get_reachable_targets_finds_all_targets` | TEST774: Tests get_reachable_targets() returns all reachable targets Verifies that reachable targets include direct and multi-step targets | src/planner/live_cap_graph.rs:1174 |
| test777 | `test777_type_mismatch_pdf_cap_does_not_match_png_input` | TEST777: Tests type checking prevents using PDF-specific cap with PNG input Verifies that media type compatibility is enforced during pathfinding | src/planner/live_cap_graph.rs:1198 |
| test778 | `test778_type_mismatch_png_cap_does_not_match_pdf_input` | TEST778: Tests type checking prevents using PNG-specific cap with PDF input Verifies that media type compatibility is enforced during pathfinding | src/planner/live_cap_graph.rs:1217 |
| test779 | `test779_get_reachable_targets_respects_type_matching` | TEST779: Tests get_reachable_targets() only returns targets reachable via type-compatible caps Verifies that PNG and PDF inputs reach different targets based on cap input type requirements | src/planner/live_cap_graph.rs:1236 |
| test780 | `test780_split_integer_array` | TEST780: split_cbor_array splits a simple array of integers | src/orchestrator/cbor_util.rs:152 |
| test781 | `test781_find_paths_respects_type_chain` | TEST781: Tests find_paths_to_exact_target() enforces type compatibility across multi-step chains Verifies that paths are only found when all intermediate types are compatible | src/planner/live_cap_graph.rs:1261 |
| test782 | `test782_split_non_array` | TEST782: split_cbor_array rejects non-array input | src/orchestrator/cbor_util.rs:193 |
| test783 | `test783_split_empty_array` | TEST783: split_cbor_array rejects empty array | src/orchestrator/cbor_util.rs:203 |
| test784 | `test784_split_invalid_cbor` | TEST784: split_cbor_array rejects invalid CBOR bytes | src/orchestrator/cbor_util.rs:213 |
| test785 | `test785_assemble_integer_array` | TEST785: assemble_cbor_array creates array from individual items | src/orchestrator/cbor_util.rs:220 |
| test786 | `test786_roundtrip_split_assemble` | TEST786: split then assemble roundtrip preserves data | src/orchestrator/cbor_util.rs:244 |
| test787 | `test787_find_paths_sorting_prefers_shorter` | TEST787: Tests find_paths_to_exact_target() sorts paths by length, preferring shorter ones Verifies that among multiple paths, the shortest is ranked first | src/planner/live_cap_graph.rs:1444 |
| test788 | `test788_foreach_edges_inserted_for_list_to_singular` | TEST788: Tests that ForEach edges are inserted for list→singular transitions This is crucial for paths like: pdf → disbind → page;list → ForEach → page → analyze | src/planner/live_cap_graph.rs:1286 |
| test789 | `test789_cap_from_json_has_valid_specs` | TEST789: Tests that caps loaded from JSON have correct in_spec/out_spec | src/planner/live_cap_graph.rs:1417 |
| test790 | `test790_identity_urn_is_specific` | TEST790: Tests identity_urn is specific and doesn't match everything | src/planner/live_cap_graph.rs:1389 |
| test791 | `test791_sync_from_cap_urns_adds_edges` | TEST791: Tests sync_from_cap_urns actually adds edges | src/planner/live_cap_graph.rs:1344 |
| test792 | `test792_argument_binding_requires_input` | TEST792: Tests ArgumentBinding requires_input distinguishes Slots from Literals Verifies Slot returns true (needs user input) while Literal returns false | src/planner/argument_binding.rs:556 |
| test793 | `test793_argument_binding_serialization` | TEST793: Tests ArgumentBinding PreviousOutput serializes/deserializes correctly Verifies JSON round-trip preserves node_id and output_field values | src/planner/argument_binding.rs:566 |
| test794 | `test794_argument_bindings_add_file_path` | TEST794: Tests ArgumentBindings add_file_path adds InputFilePath binding Verifies add_file_path() creates binding map entry with InputFilePath variant | src/planner/argument_binding.rs:586 |
| test795 | `test795_argument_bindings_unresolved_slots` | TEST795: Tests ArgumentBindings identifies unresolved Slot bindings Verifies has_unresolved_slots() and get_unresolved_slots() detect Slots needing values | src/planner/argument_binding.rs:596 |
| test796 | `test796_resolve_input_file_path` | TEST796: Tests resolve_binding resolves InputFilePath to current file path Verifies InputFilePath binding resolves to file path bytes with InputFile source | src/planner/argument_binding.rs:607 |
| test797 | `test797_resolve_literal` | TEST797: Tests resolve_binding resolves Literal to JSON-encoded bytes Verifies Literal binding serializes value to bytes with Literal source | src/planner/argument_binding.rs:627 |
| test798 | `test798_resolve_previous_output` | TEST798: Tests resolve_binding extracts value from previous node output Verifies PreviousOutput binding fetches field from earlier execution results | src/planner/argument_binding.rs:647 |
| test799 | `test799_cap_chain_input_single` | TEST799: Tests CapChainInput single constructor creates valid Single cardinality input Verifies single() wraps one file with Single cardinality and validates correctly | src/planner/argument_binding.rs:671 |
| test800 | `test800_cap_chain_input_vector` | TEST800: Tests CapChainInput sequence constructor creates valid Sequence cardinality input Verifies sequence() wraps multiple files with Sequence cardinality | src/planner/argument_binding.rs:682 |
| test801 | `test801_cap_input_file_deserialization_from_dry_context` | TEST801: Tests CapInputFile deserializes from JSON with source metadata fields Verifies JSON with source_id and source_type deserializes to CapInputFile correctly | src/planner/argument_binding.rs:696 |
| test802 | `test802_cap_input_file_deserialization_via_value` | TEST802: Tests CapInputFile deserializes from compact JSON via serde_json::Value Verifies deserialization through Value intermediate works correctly | src/planner/argument_binding.rs:715 |
| test803 | `test803_cap_chain_input_invalid_single` | TEST803: Tests CapChainInput validation detects mismatched Single cardinality with multiple files Verifies is_valid() returns false when Single cardinality has more than one file | src/planner/argument_binding.rs:821 |
| test804 | `test804_extract_json_path_simple` | TEST804: Tests basic JSON path extraction with dot notation for nested objects Verifies that simple paths like "data.message" correctly extract values from nested JSON structures | src/planner/executor.rs:558 |
| test805 | `test805_extract_json_path_with_array` | TEST805: Tests JSON path extraction with array indexing syntax Verifies that bracket notation like "items[0].name" correctly accesses array elements and their nested fields | src/planner/executor.rs:572 |
| test806 | `test806_extract_json_path_missing_field` | TEST806: Tests error handling when JSON path references non-existent fields Verifies that accessing missing fields returns an appropriate error message | src/planner/executor.rs:587 |
| test807 | `test807_apply_edge_type_direct` | TEST807: Tests EdgeType::Direct passes JSON values through unchanged Verifies that Direct edge type acts as a transparent passthrough without transformation | src/planner/executor.rs:598 |
| test808 | `test808_apply_edge_type_json_field` | TEST808: Tests EdgeType::JsonField extracts specific top-level fields from JSON objects Verifies that JsonField edge type correctly isolates a single named field from the source output | src/planner/executor.rs:608 |
| test809 | `test809_apply_edge_type_json_field_missing` | TEST809: Tests EdgeType::JsonField error handling for missing fields Verifies that attempting to extract a non-existent field returns an error | src/planner/executor.rs:618 |
| test810 | `test810_apply_edge_type_json_path` | TEST810: Tests EdgeType::JsonPath extracts values using nested path expressions Verifies that JsonPath edge type correctly navigates through multiple levels like "data.nested.value" | src/planner/executor.rs:627 |
| test811 | `test811_apply_edge_type_iteration` | TEST811: Tests EdgeType::Iteration preserves array values for iterative processing Verifies that Iteration edge type passes through arrays unchanged to enable ForEach patterns | src/planner/executor.rs:637 |
| test812 | `test812_apply_edge_type_collection` | TEST812: Tests EdgeType::Collection preserves collected values without transformation Verifies that Collection edge type maintains structure for aggregation patterns | src/planner/executor.rs:647 |
| test813 | `test813_extract_json_path_deeply_nested` | TEST813: Tests JSON path extraction through deeply nested object hierarchies (4+ levels) Verifies that paths can traverse multiple nested levels like "level1.level2.level3.level4.value" | src/planner/executor.rs:657 |
| test814 | `test814_extract_json_path_array_out_of_bounds` | TEST814: Tests error handling when array index exceeds available elements Verifies that out-of-bounds array access returns a descriptive error message | src/planner/executor.rs:677 |
| test815 | `test815_extract_json_path_single_segment` | TEST815: Tests JSON path extraction with single-level paths (no nesting) Verifies that simple field names without dots correctly extract top-level values | src/planner/executor.rs:690 |
| test816 | `test816_extract_json_path_with_special_characters` | TEST816: Tests JSON path extraction preserves special characters in string values Verifies that quotes, backslashes, and other special characters are correctly maintained | src/planner/executor.rs:700 |
| test817 | `test817_extract_json_path_with_null_value` | TEST817: Tests JSON path extraction correctly handles explicit null values Verifies that null is returned as serde_json::Value::Null rather than an error | src/planner/executor.rs:714 |
| test818 | `test818_extract_json_path_with_empty_array` | TEST818: Tests JSON path extraction correctly returns empty arrays Verifies that zero-length arrays are extracted as valid empty array values | src/planner/executor.rs:724 |
| test819 | `test819_extract_json_path_with_numeric_types` | TEST819: Tests JSON path extraction handles various numeric types correctly Verifies extraction of integers, floats, negative numbers, and zero | src/planner/executor.rs:734 |
| test820 | `test820_extract_json_path_with_boolean` | TEST820: Tests JSON path extraction correctly handles boolean values Verifies that true and false are extracted as proper boolean JSON values | src/planner/executor.rs:750 |
| test821 | `test821_extract_json_path_with_nested_arrays` | TEST821: Tests JSON path extraction with multi-dimensional arrays (matrix access) Verifies that nested array structures like "matrix[1]" correctly extract inner arrays | src/planner/executor.rs:764 |
| test822 | `test822_extract_json_path_invalid_array_index` | TEST822: Tests error handling for non-numeric array indices Verifies that invalid indices like "items[abc]" return a descriptive parse error | src/planner/executor.rs:779 |
| test823 | `test823_dispatch_exact_match` | TEST823: is_dispatchable — exact match provider dispatches request | src/urn/cap_urn.rs:2223 |
| test824 | `test824_dispatch_contravariant_input` | TEST824: is_dispatchable — provider with broader input handles specific request (contravariance) | src/urn/cap_urn.rs:2235 |
| test825 | `test825_dispatch_request_unconstrained_input` | TEST825: is_dispatchable — request with unconstrained input dispatches to specific provider media: on the request input axis means "unconstrained" — vacuously true | src/urn/cap_urn.rs:2248 |
| test826 | `test826_dispatch_covariant_output` | TEST826: is_dispatchable — provider output must satisfy request output (covariance) | src/urn/cap_urn.rs:2261 |
| test827 | `test827_dispatch_generic_output_fails` | TEST827: is_dispatchable — provider with generic output cannot satisfy specific request | src/urn/cap_urn.rs:2274 |
| test828 | `test828_dispatch_wildcard_requires_tag_presence` | TEST828: is_dispatchable — wildcard * tag in request, provider missing tag → reject | src/urn/cap_urn.rs:2287 |
| test829 | `test829_dispatch_wildcard_with_tag_present` | TEST829: is_dispatchable — wildcard * tag in request, provider has tag → accept | src/urn/cap_urn.rs:2300 |
| test830 | `test830_dispatch_provider_extra_tags` | TEST830: is_dispatchable — provider extra tags are refinement, always OK | src/urn/cap_urn.rs:2313 |
| test831 | `test831_dispatch_cross_backend_mismatch` | TEST831: is_dispatchable — cross-backend mismatch prevented | src/urn/cap_urn.rs:2326 |
| test832 | `test832_dispatch_asymmetric` | TEST832: is_dispatchable is NOT symmetric | src/urn/cap_urn.rs:2339 |
| test833 | `test833_comparable_symmetric` | TEST833: is_comparable — both directions checked | src/urn/cap_urn.rs:2359 |
| test834 | `test834_comparable_unrelated` | TEST834: is_comparable — unrelated caps are NOT comparable | src/urn/cap_urn.rs:2372 |
| test835 | `test835_equivalent_identical` | TEST835: is_equivalent — identical caps | src/urn/cap_urn.rs:2385 |
| test836 | `test836_equivalent_non_equivalent` | TEST836: is_equivalent — non-equivalent comparable caps | src/urn/cap_urn.rs:2398 |
| test837 | `test837_dispatch_op_mismatch` | TEST837: is_dispatchable — op tag mismatch rejects | src/urn/cap_urn.rs:2411 |
| test838 | `test838_dispatch_request_wildcard_output` | TEST838: is_dispatchable — request with wildcard output accepts any provider output | src/urn/cap_urn.rs:2423 |
| test850 | `test850_with_list_without_list` | TEST850: with_list adds list marker, without_list removes it | src/urn/media_urn.rs:1053 |
| test851 | `test851_with_list_idempotent` | TEST851: with_list is idempotent | src/urn/media_urn.rs:1071 |
| test852 | `test852_lub_identical` | TEST852: LUB of identical URNs returns the same URN | src/urn/media_urn.rs:1082 |
| test853 | `test853_lub_no_common_tags` | TEST853: LUB of URNs with no common tags returns media: (universal) | src/urn/media_urn.rs:1090 |
| test854 | `test854_lub_partial_overlap` | TEST854: LUB keeps common tags, drops differing ones | src/urn/media_urn.rs:1101 |
| test855 | `test855_lub_list_vs_scalar` | TEST855: LUB of list and non-list drops list tag | src/urn/media_urn.rs:1112 |
| test856 | `test856_lub_empty` | TEST856: LUB of empty input returns universal type | src/urn/media_urn.rs:1123 |
| test857 | `test857_lub_single` | TEST857: LUB of single input returns that input | src/urn/media_urn.rs:1131 |
| test858 | `test858_lub_three_inputs` | TEST858: LUB with three+ inputs narrows correctly | src/urn/media_urn.rs:1139 |
| test859 | `test859_lub_valued_tags` | TEST859: LUB with valued tags (non-marker) that differ | src/urn/media_urn.rs:1151 |
| test890 | `test890_direction_semantic_matching` | TEST890: Semantic direction matching - generic provider matches specific request | src/urn/cap_urn.rs:1744 |
| test891 | `test891_direction_semantic_specificity` | TEST891: Semantic direction specificity - more media URN tags = higher specificity | src/urn/cap_urn.rs:1801 |
| test892 | `test892_extensions_serialization` | TEST892: Test extensions serializes/deserializes correctly in MediaSpecDef | src/media/spec.rs:1073 |
| test893 | `test893_extensions_with_metadata_and_validation` | TEST893: Test extensions can coexist with metadata and validation | src/media/spec.rs:1095 |
| test894 | `test894_multiple_extensions` | TEST894: Test multiple extensions in a media spec | src/media/spec.rs:1130 |
| test895 | `test895_cbor_array_file_paths_in_cbor_mode` | TEST895: CBOR Array of file-paths in CBOR mode (validates new Array support) | src/bifaci/plugin_runtime.rs:5273 |
| test896 | `test896_full_path_engine_req_to_plugin_response` | TEST896: Full path: engine REQ → runtime → plugin → response back through relay | src/bifaci/integration_tests.rs:159 |
| test897 | `test897_plugin_error_flows_to_engine` | TEST897: Plugin ERR frame flows back to engine through relay | src/bifaci/integration_tests.rs:268 |
| test898 | `test898_binary_integrity_through_relay` | TEST898: Binary data integrity through full relay path (256 byte values) | src/bifaci/integration_tests.rs:340 |
| test899 | `test899_streaming_chunks_through_relay` | TEST899: Streaming chunks flow through relay without accumulation | src/bifaci/integration_tests.rs:454 |
| test900 | `test900_two_plugins_routed_independently` | TEST900: Two plugins routed independently by cap_urn | src/bifaci/integration_tests.rs:550 |
| test901 | `test901_req_for_unknown_cap_returns_err_frame` | TEST901: REQ for unknown cap returns ERR frame (not fatal) | src/bifaci/integration_tests.rs:683 |
| test902 | `test902_compute_checksum_empty` | TEST902: Verify FNV-1a checksum handles empty data | src/bifaci/frame.rs:1443 |
| test903 | `test903_chunk_with_chunk_index_and_checksum` | TEST903: Verify CHUNK frame can store chunk_index and checksum fields | src/bifaci/frame.rs:1451 |
| test904 | `test904_stream_end_with_chunk_count` | TEST904: Verify STREAM_END frame can store chunk_count field | src/bifaci/frame.rs:1469 |
| test905 | `test905_send_to_master_build_request_frames_roundtrip` | TEST905: send_to_master + build_request_frames through RelaySwitch → RelaySlave → InProcessPluginHost roundtrip | src/bifaci/relay_switch.rs:2332 |
| test906 | `test906_full_path_identity_verification` | TEST489: Full path identity verification: engine → host (attach_plugin) → plugin  This verifies that attach_plugin completes identity verification end-to-end and the plugin is ready to handle subsequent requests. | src/bifaci/integration_tests.rs:1138 |
| test907 | `test907_cbor_rejects_stream_end_without_chunk_count` | TEST907: CBOR decode REJECTS STREAM_END frame missing chunk_count field | src/bifaci/frame.rs:1998 |
| test908 | `test908_map_progress_basic_mapping` | TEST908: map_progress clamps child to [0.0, 1.0] and maps to [base, base+weight] | src/orchestrator/executor.rs:1263 |
| test909 | `test909_map_progress_deterministic` | TEST909: map_progress is deterministic — same inputs always produce same output | src/orchestrator/executor.rs:1281 |
| test910 | `test910_map_progress_monotonic` | TEST910: map_progress output is monotonic for monotonically increasing input | src/orchestrator/executor.rs:1292 |
| test911 | `test911_map_progress_bounded` | TEST911: map_progress output is bounded within [base, base+weight] | src/orchestrator/executor.rs:1308 |
| test912 | `test912_progress_mapper_reports_through_parent` | TEST912: ProgressMapper correctly maps through a CapProgressFn | src/orchestrator/executor.rs:1324 |
| test913 | `test913_progress_mapper_as_cap_progress_fn` | TEST913: ProgressMapper.as_cap_progress_fn produces same mapping | src/orchestrator/executor.rs:1345 |
| test914 | `test914_progress_mapper_sub_mapper` | TEST914: ProgressMapper.sub_mapper chains correctly | src/orchestrator/executor.rs:1368 |
| test915 | `test915_per_group_subdivision_monotonic_bounded` | TEST915: Per-group subdivision produces monotonic, bounded progress for N groups  Uses pre-computed boundaries (same pattern as production code) to guarantee monotonicity regardless of f32 rounding. | src/orchestrator/executor.rs:1395 |
| test916 | `test916_foreach_item_subdivision` | TEST916: ForEach item subdivision produces correct, monotonic ranges  Mirrors the production code in interpreter.rs: pre-compute item boundaries from the same formula so the end of item N and the start of item N+1 are the same f32 value (no divergent accumulation paths). | src/orchestrator/executor.rs:1451 |
| test917 | `test917_high_frequency_progress_bounded` | TEST917: High-frequency progress emission does not violate bounds (Regression test for the deadlock scenario — verifies computation stays bounded) | src/orchestrator/executor.rs:1498 |
| test918 | `test918_activity_timeout_error_display` | TEST918: ActivityTimeout error formats correctly | src/orchestrator/executor.rs:1531 |
| test920 | `test920_single_cap_plan` | TEST920: Tests creation of a simple execution plan with a single capability Verifies that single_cap() generates a valid plan with input_slot, cap node, and output node | src/planner/plan.rs:938 |
| test921 | `test921_linear_chain_plan` | TEST921: Tests creation of a linear chain of capabilities connected in sequence Verifies that linear_chain() correctly links multiple caps with proper edges and topological order | src/planner/plan.rs:954 |
| test922 | `test922_empty_plan` | TEST922: Tests creation and validation of an empty execution plan with no nodes Verifies that plans without capabilities are valid and handle zero nodes correctly | src/planner/plan.rs:972 |
| test923 | `test923_plan_with_metadata` | TEST923: Tests storing and retrieving metadata attached to an execution plan Verifies that arbitrary JSON metadata can be associated with a plan for context preservation | src/planner/plan.rs:981 |
| test924 | `test924_validate_invalid_edge` | TEST924: Tests plan validation detects edges pointing to non-existent nodes Verifies that validate() returns an error when an edge references a missing to_node | src/planner/plan.rs:998 |
| test925 | `test925_topological_order_diamond` | TEST925: Tests topological sort correctly orders a diamond-shaped DAG (A->B,C->D) Verifies that nodes with multiple paths respect dependency constraints (A first, D last) | src/planner/plan.rs:1015 |
| test926 | `test926_topological_order_detects_cycle` | TEST926: Tests topological sort detects and rejects cyclic dependencies (A->B->C->A) Verifies that circular references produce a "Cycle detected" error | src/planner/plan.rs:1041 |
| test927 | `test927_execution_result` | TEST927: Tests CapChainExecutionResult structure for successful execution outcomes Verifies that success status, outputs, and primary_output() accessor work correctly | src/planner/plan.rs:1101 |
| test928 | `test928_validate_invalid_from_node` | TEST928: Tests plan validation detects edges originating from non-existent nodes Verifies that validate() returns an error when an edge references a missing from_node | src/planner/plan.rs:1120 |
| test929 | `test929_validate_invalid_entry_node` | TEST929: Tests plan validation detects invalid entry node references Verifies that validate() returns an error when entry_nodes contains a non-existent node ID | src/planner/plan.rs:1137 |
| test930 | `test930_validate_invalid_output_node` | TEST930: Tests plan validation detects invalid output node references Verifies that validate() returns an error when output_nodes contains a non-existent node ID | src/planner/plan.rs:1155 |
| test931 | `test931_node_execution_result_failure` | TEST931: Tests NodeExecutionResult structure for failed node execution Verifies that failure status, error message, and absence of outputs are correctly represented | src/planner/plan.rs:1312 |
| test932 | `test932_execution_result_failure` | TEST932: Tests CapChainExecutionResult structure for failed chain execution Verifies that failure status, error message, and absence of outputs are correctly represented | src/planner/plan.rs:1330 |
| test933 | `test933_serialization_roundtrip` | TEST933: Tests CapInputCollection serializes to JSON and deserializes correctly Verifies JSON round-trip preserves folder_id, folder_name, files and file metadata | src/planner/collection_input.rs:265 |
| test934 | `test934_find_first_foreach` | TEST934: find_first_foreach detects ForEach in a plan | src/planner/plan.rs:1542 |
| test935 | `test935_find_first_foreach_linear` | TEST935: find_first_foreach returns None for linear plans | src/planner/plan.rs:1550 |
| test936 | `test936_has_foreach_or_collect` | TEST936: has_foreach_or_collect detects ForEach/Collect | src/planner/plan.rs:1562 |
| test937 | `test937_extract_prefix_to` | TEST937: extract_prefix_to extracts input_slot -> cap_0 as a standalone plan | src/planner/plan.rs:1577 |
| test938 | `test938_no_duplicates_with_unique_caps` | TEST938: Tests duplicate detection passes for caps with unique URN combinations Verifies that check_for_duplicate_caps() correctly accepts caps with different op/in/out combinations | src/planner/plan_builder.rs:776 |
| test939 | `test939_detects_duplicate_cap_urns` | TEST939: Tests duplicate detection identifies caps with identical URNs Verifies that check_for_duplicate_caps() returns an error when multiple caps share the same cap_urn | src/planner/plan_builder.rs:792 |
| test940 | `test940_different_ops_same_types_not_duplicates` | TEST940: Tests caps with different operations but same input/output types are not duplicates Verifies that only the complete URN (including op) is used for duplicate detection | src/planner/plan_builder.rs:810 |
| test941 | `test941_same_op_different_input_types_not_duplicates` | TEST941: Tests caps with same operation but different input types are not duplicates Verifies that input type differences distinguish caps with the same operation name | src/planner/plan_builder.rs:825 |
| test942 | `test942_input_arg_first_cap_auto_resolved_from_input` | TEST942: Tests first cap's input argument is automatically resolved from input file Verifies that determine_resolution_with_io_check() returns FromInputFile for the first cap in a chain | src/planner/plan_builder.rs:865 |
| test943 | `test943_input_arg_subsequent_cap_auto_resolved_from_previous` | TEST943: Tests subsequent caps' input arguments are automatically resolved from previous output Verifies that determine_resolution_with_io_check() returns FromPreviousOutput for caps after the first | src/planner/plan_builder.rs:876 |
| test944 | `test944_output_arg_auto_resolved` | TEST944: Tests output arguments are automatically resolved from previous cap's output Verifies that arguments matching the output spec are always resolved as FromPreviousOutput | src/planner/plan_builder.rs:891 |
| test945 | `test945_file_path_type_fallback_first_cap` | TEST945: Tests MEDIA_FILE_PATH argument type resolves to input file for first cap Verifies that generic file-path arguments are bound to input file in the first cap | src/planner/plan_builder.rs:902 |
| test946 | `test946_file_path_type_fallback_subsequent_cap` | TEST946: Tests MEDIA_FILE_PATH argument type resolves to previous output for subsequent caps Verifies that generic file-path arguments are bound to previous cap's output after the first cap | src/planner/plan_builder.rs:913 |
| test947 | `test947_file_path_array_fallback` | TEST947: Tests MEDIA_FILE_PATH_ARRAY argument type resolution for first and subsequent caps Verifies that file-path array arguments follow the same resolution pattern as single file paths | src/planner/plan_builder.rs:924 |
| test948 | `test948_non_io_arg_with_default_has_default` | TEST948: Tests required non-IO arguments with default values are marked as HasDefault Verifies that arguments like integers with defaults don't require user input | src/planner/plan_builder.rs:938 |
| test949 | `test949_non_io_arg_without_default_requires_user_input` | TEST949: Tests required non-IO arguments without defaults require user input Verifies that arguments like strings without defaults are marked as RequiresUserInput | src/planner/plan_builder.rs:950 |
| test950 | `test950_optional_non_io_arg_with_default_has_default` | TEST950: Tests optional non-IO arguments with default values are marked as HasDefault Verifies that optional arguments with defaults behave the same as required ones with defaults | src/planner/plan_builder.rs:961 |
| test951 | `test951_optional_non_io_arg_without_default_requires_user_input` | TEST951: Tests optional non-IO arguments without defaults still require user input Verifies that optional arguments without defaults must be explicitly provided or skipped | src/planner/plan_builder.rs:973 |
| test952 | `test952_validation_to_json_none` | TEST952: Tests validation_to_json() returns None for None input Verifies that missing validation metadata is converted to JSON None | src/planner/plan_builder.rs:984 |
| test953 | `test953_linear_plan_still_works` | TEST953: Linear plans (no ForEach/Collect) still convert successfully | src/orchestrator/plan_converter.rs:295 |
| test954 | `test954_wrap_in_list_passthrough` | TEST954: WrapInList nodes are handled as pass-through Plan: input → cap_0 → WrapInList → cap_1 → output The WrapInList is transparent — the resolved edge from WrapInList to cap_1 should be rewritten to go from cap_0 to cap_1 directly. | src/orchestrator/plan_converter.rs:317 |
| test955 | `test955_split_map_array` | TEST955: split_cbor_array with nested maps | src/orchestrator/cbor_util.rs:172 |
| test956 | `test956_roundtrip_assemble_split` | TEST956: assemble then split roundtrip preserves data | src/orchestrator/cbor_util.rs:263 |
| test957 | `test957_cap_input_file_new` | TEST957: Tests CapInputFile constructor creates file with correct path and media URN Verifies new() initializes file_path, media_urn and leaves metadata/source_id as None | src/planner/argument_binding.rs:516 |
| test958 | `test958_cap_input_file_from_listing` | TEST958: Tests CapInputFile from_listing sets source metadata correctly Verifies from_listing() populates source_id and source_type as Listing | src/planner/argument_binding.rs:527 |
| test959 | `test959_cap_input_file_filename` | TEST959: Tests CapInputFile extracts filename from full path correctly Verifies filename() returns just the basename without directory path | src/planner/argument_binding.rs:536 |
| test960 | `test960_argument_binding_literal_string` | TEST960: Tests ArgumentBinding literal_string creates Literal variant with string value Verifies literal_string() wraps string in JSON Value::String | src/planner/argument_binding.rs:544 |
| test961 | `test961_assemble_empty` | TEST961: assemble empty list produces empty CBOR array | src/orchestrator/cbor_util.rs:279 |
| test962 | `test962_assemble_invalid_item` | TEST962: assemble rejects invalid CBOR item | src/orchestrator/cbor_util.rs:287 |
| test963 | `test963_split_binary_items` | TEST963: split preserves CBOR byte strings (binary data — the common case in bifaci) | src/orchestrator/cbor_util.rs:299 |
| test964 | `test964_split_sequence_bytes` | TEST964: split_cbor_sequence splits concatenated CBOR Bytes values | src/orchestrator/cbor_util.rs:333 |
| test965 | `test965_split_sequence_text` | TEST965: split_cbor_sequence splits concatenated CBOR Text values | src/orchestrator/cbor_util.rs:357 |
| test966 | `test966_split_sequence_mixed` | TEST966: split_cbor_sequence handles mixed types | src/orchestrator/cbor_util.rs:374 |
| test967 | `test967_split_sequence_single` | TEST967: split_cbor_sequence single-item sequence | src/orchestrator/cbor_util.rs:395 |
| test968 | `test968_roundtrip_assemble_split_sequence` | TEST968: roundtrip — assemble then split preserves items | src/orchestrator/cbor_util.rs:408 |
| test969 | `test969_roundtrip_split_assemble_sequence` | TEST969: roundtrip — split then assemble preserves byte-for-byte | src/orchestrator/cbor_util.rs:427 |
| test970 | `test970_split_sequence_empty` | TEST970: split_cbor_sequence rejects empty data | src/orchestrator/cbor_util.rs:441 |
| test971 | `test971_split_sequence_truncated` | TEST971: split_cbor_sequence rejects truncated CBOR | src/orchestrator/cbor_util.rs:448 |
| test972 | `test972_assemble_sequence_invalid_item` | TEST972: assemble_cbor_sequence rejects invalid CBOR item | src/orchestrator/cbor_util.rs:464 |
| test973 | `test973_assemble_sequence_empty` | TEST973: assemble_cbor_sequence with empty items list produces empty bytes | src/orchestrator/cbor_util.rs:476 |
| test974 | `test974_sequence_is_not_array` | TEST974: CBOR sequence is NOT a CBOR array — split_cbor_array rejects a sequence | src/orchestrator/cbor_util.rs:483 |
| test975 | `test975_single_value_sequence` | TEST975: split_cbor_sequence works on data that is also a valid CBOR array (single top-level value) | src/orchestrator/cbor_util.rs:498 |
| test976 | `test976_cap_graph_find_best_path` | TEST976: CapGraph::find_best_path returns highest-specificity path over shortest | src/urn/cap_matrix.rs:1807 |
| test977 | `test977_os_files_excluded_integration` | TEST977 (integration): OS files excluded in resolve_paths | src/input_resolver/resolver.rs:378 |
| test978 | `test978_resolve_json_object` | TEST978 (integration): JSON object via resolve_paths | src/input_resolver/resolver.rs:304 |
| test979 | `test979_resolve_json_array_of_objects` | TEST979 (integration): JSON array of objects via resolve_paths | src/input_resolver/resolver.rs:319 |
| test980 | `test980_resolve_ndjson` | TEST980 (integration): NDJSON via resolve_paths | src/input_resolver/resolver.rs:335 |
| test981 | `test981_resolve_yaml_mapping` | TEST981 (integration): YAML mapping via resolve_paths | src/input_resolver/resolver.rs:350 |
| test982 | `test982_resolve_yaml_sequence` | TEST982 (integration): YAML sequence via resolve_paths | src/input_resolver/resolver.rs:364 |
| test983 | `test983_json_detection_via_adapter_registry` | TEST983 (registry integration): JSON detection via MediaAdapterRegistry | input_resolver/adapters/registry.rs:225 |
| test1000 | `test1000_single_existing_file` | TEST1000: Single existing file | src/input_resolver/path_resolver.rs:256 |
| test1001 | `test1001_nonexistent_file` | TEST1001: Single non-existent file | src/input_resolver/path_resolver.rs:268 |
| test1002 | `test1002_empty_directory` | TEST1002: Empty directory | src/input_resolver/path_resolver.rs:275 |
| test1003 | `test1003_directory_with_files` | TEST1003: Directory with files | src/input_resolver/path_resolver.rs:284 |
| test1004 | `test1004_directory_with_subdirs` | TEST1004: Directory with subdirs (recursive) | src/input_resolver/path_resolver.rs:296 |
| test1005 | `test1005_glob_matching_files` | TEST1005: Glob matching files | src/input_resolver/path_resolver.rs:308 |
| test1006 | `test1006_glob_matching_nothing` | TEST1006: Glob matching nothing | src/input_resolver/path_resolver.rs:321 |
| test1007 | `test1007_recursive_glob` | TEST1007: Recursive glob | src/input_resolver/path_resolver.rs:332 |
| test1008 | `test1008_mixed_file_dir` | TEST1008: Mixed file + dir | src/input_resolver/path_resolver.rs:345 |
| test1010 | `test1010_duplicate_paths` | TEST1010: Duplicate paths are deduplicated | src/input_resolver/path_resolver.rs:363 |
| test1011 | `test1011_invalid_glob` | TEST1011: Invalid glob syntax | src/input_resolver/path_resolver.rs:379 |
| test1013 | `test1013_empty_input` | TEST1013: Empty input array | src/input_resolver/path_resolver.rs:386 |
| test1014 | `test1014_symlink_to_file` |  | src/input_resolver/path_resolver.rs:394 |
| test1016 | `test1016_path_with_spaces` | TEST1016: Path with spaces | src/input_resolver/path_resolver.rs:409 |
| test1017 | `test1017_path_with_unicode` | TEST1017: Path with unicode | src/input_resolver/path_resolver.rs:420 |
| test1018 | `test1018_relative_path` | TEST1018: Relative path | src/input_resolver/path_resolver.rs:431 |
| test1020 | `test1020_ds_store_excluded` | TEST1020: macOS .DS_Store is excluded | src/input_resolver/os_filter.rs:162 |
| test1021 | `test1021_thumbs_db_excluded` | TEST1021: Windows Thumbs.db is excluded | src/input_resolver/os_filter.rs:169 |
| test1022 | `test1022_resource_fork_excluded` | TEST1022: macOS resource fork files are excluded | src/input_resolver/os_filter.rs:176 |
| test1023 | `test1023_office_lock_excluded` | TEST1023: Office lock files are excluded | src/input_resolver/os_filter.rs:183 |
| test1024 | `test1024_git_dir_excluded` | TEST1024: .git directory is excluded | src/input_resolver/os_filter.rs:190 |
| test1025 | `test1025_macosx_dir_excluded` | TEST1025: __MACOSX archive artifact is excluded | src/input_resolver/os_filter.rs:197 |
| test1026 | `test1026_temp_files_excluded` | TEST1026: Temp files are excluded | src/input_resolver/os_filter.rs:204 |
| test1027 | `test1027_localized_excluded` | TEST1027: .localized is excluded | src/input_resolver/os_filter.rs:213 |
| test1028 | `test1028_desktop_ini_excluded` | TEST1028: desktop.ini is excluded | src/input_resolver/os_filter.rs:219 |
| test1029 | `test1029_normal_files_not_excluded` | TEST1029: Normal files are NOT excluded | src/input_resolver/os_filter.rs:225 |
| test1030 | `test1030_json_empty_object` | TEST1030: Empty object | input_resolver/adapters/data.rs:425 |
| test1031 | `test1031_json_simple_object` | TEST1031: Simple object | input_resolver/adapters/data.rs:437 |
| test1033 | `test1033_json_empty_array` | TEST1033: Empty array | input_resolver/adapters/data.rs:448 |
| test1036 | `test1036_json_array_of_objects` | TEST1036: Array of objects | input_resolver/adapters/data.rs:460 |
| test1039 | `test1039_json_number_primitive` | TEST1039: Number primitive | input_resolver/adapters/data.rs:472 |
| test1045 | `test1045_ndjson_objects` | TEST1045: Objects only | input_resolver/adapters/data.rs:486 |
| test1047 | `test1047_ndjson_primitives` | TEST1047: Primitives only | input_resolver/adapters/data.rs:498 |
| test1055 | `test1055_csv_multi_column` | TEST1055: Multi-column with header | input_resolver/adapters/data.rs:512 |
| test1056 | `test1056_csv_single_column` | TEST1056: Single column | input_resolver/adapters/data.rs:524 |
| test1065 | `test1065_yaml_mapping` | TEST1065: Simple mapping | input_resolver/adapters/data.rs:538 |
| test1067 | `test1067_yaml_sequence_of_scalars` | TEST1067: Sequence of scalars | input_resolver/adapters/data.rs:550 |
| test1068 | `test1068_yaml_sequence_of_mappings` | TEST1068: Sequence of mappings | input_resolver/adapters/data.rs:562 |
| test1080 | `test1080_pdf_extension` | TEST1080: PDF extension mapping | input_resolver/adapters/documents.rs:228 |
| test1081 | `test1081_png_extension` | TEST1081: PNG extension mapping | input_resolver/adapters/images.rs:338 |
| test1082 | `test1082_mp3_extension` | TEST1082: MP3 extension mapping | input_resolver/adapters/audio.rs:243 |
| test1083 | `test1083_mp4_extension` | TEST1083: MP4 extension mapping | input_resolver/adapters/video.rs:266 |
| test1084 | `test1084_rust_extension` | TEST1084: Rust code extension mapping | input_resolver/adapters/code.rs:734 |
| test1085 | `test1085_python_extension` | TEST1085: Python code extension mapping | input_resolver/adapters/code.rs:745 |
| test1087 | `test1087_toml_always_record` | TEST1087: TOML always record | input_resolver/adapters/data.rs:574 |
| test1089 | `test1089_unknown_extension` | TEST1089: Unknown extension fallback | input_resolver/adapters/other.rs:499 |
| test1090 | `test1090_single_file_scalar` | TEST1090: 1 file scalar content | src/input_resolver/resolver.rs:199 |
| test1091 | `test1091_single_file_list_content` | TEST1091: 1 file list content (CSV) | src/input_resolver/resolver.rs:211 |
| test1092 | `test1092_two_files` | TEST1092: 2 files | src/input_resolver/resolver.rs:230 |
| test1093 | `test1093_dir_single_file` | TEST1093: 1 dir with 1 file | src/input_resolver/resolver.rs:247 |
| test1094 | `test1094_dir_multiple_files` | TEST1094: 1 dir with 3 files | src/input_resolver/resolver.rs:259 |
| test1095 | `test1095_glob_with_detection` | TEST1095/1096 (integration): Glob with detection | src/input_resolver/resolver.rs:391 |
| test1098 | `test1098_common_media` | TEST1098: Common media (all same type) | src/input_resolver/resolver.rs:276 |
| test1099 | `test1099_heterogeneous` | TEST1099: Heterogeneous (mixed types) | src/input_resolver/resolver.rs:289 |
| test1100 | `test1100_cap_urn_normalizes_media_urn_tag_order` | TEST1100: Tests that CapUrn normalizes media URN tags to canonical order This is the root cause fix for caps not matching when plugins report URNs with different tag ordering than the registry (e.g., "record;textable" vs "textable;record") | src/planner/plan_builder.rs:1137 |
| test1103 | `test1103_is_dispatchable_uses_correct_directionality` | TEST1103: Tests that is_dispatchable has correct directionality The available cap (provider) must be dispatchable for the requested cap (request). This tests the directionality: provider.is_dispatchable(&request) NOTE: This now tests CapUrn::is_dispatchable directly, not via CapPlanBuilder | src/planner/plan_builder.rs:1164 |
| test1104 | `test1104_is_dispatchable_rejects_non_dispatchable` | TEST1104: Tests that is_dispatchable rejects when provider cannot dispatch request | src/planner/plan_builder.rs:1189 |
| test1105 | `test1105_two_steps_same_cap_urn_different_slot_values` | TEST1105: Two steps with the same cap_urn get distinct slot values via different node_ids. This is the core disambiguation scenario that step-index keying was designed to solve. | src/planner/argument_binding.rs:837 |
| test1106 | `test1106_slot_falls_through_to_cap_settings_shared` | TEST1106: Slot resolution falls through to cap_settings when no slot_value exists. cap_settings are keyed by cap_urn (shared across steps), so both steps get the same value. | src/planner/argument_binding.rs:878 |
| test1107 | `test1107_slot_value_overrides_cap_settings_per_step` | TEST1107: step_0 has a slot_value override, step_1 falls through to cap_settings. Proves per-step override works while shared settings remain as fallback. | src/planner/argument_binding.rs:915 |
| test1108 | `test1108_resolve_all_passes_node_id` | TEST1108: ResolveAll with node_id threads correctly through to each binding. | src/planner/argument_binding.rs:958 |
| test1109 | `test1109_slot_key_uses_node_id_not_cap_urn` | TEST1109: Slot key uses node_id, NOT cap_urn — a slot_value keyed by cap_urn must not match. | src/planner/argument_binding.rs:995 |

---

*Generated from capdag source tree*
*Total numbered tests: 923*
