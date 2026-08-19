#[macro_use]
mod utils;

use pinny::tag;

#[tag(tag1)]
#[test]
fn test_with_1_tag() {
    assert_eq!("test_with_1_tag::t::tag1::t", function_path!());
}

#[tag(tag1, tag2)]
#[test]
fn test_with_2_tags() {
    assert_eq!("test_with_2_tags::t::tag1::tag2::t", function_path!());
}

const fn invoke_me() {}

#[tag(tag2)]
#[test]
fn test_invoke_outer_fn() {
    invoke_me();
}

#[test]
fn test_refer_to_tagged_test_original_name() {
    assert_eq!(
        "test_invoke_outer_fn::t::tag2::t",
        function_path!(test_invoke_outer_fn)
    );
}

#[tag(tag1)]
#[test]
#[ignore = "verifies that #[tag] preserves #[ignore]; must never run"]
fn test_ignored_tagged_test() {
    panic!();
}
