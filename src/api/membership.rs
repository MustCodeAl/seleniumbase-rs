//! What `assert_in` and `assert_not_in` can look in.
//!
//! Python's `assert_in(first, second)` asserts `first in second`: an element of
//! a list, a tuple or a set, or a substring of a string. [`Contains`] says which
//! of those a Rust container is, so one method takes both:
//!
//! ```
//! # use seleniumbase_rs::api::base_case::BaseCase;
//! # use seleniumbase_rs::browser::config::BrowserConfig;
//! # fn demo(sb: &BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! sb.assert_in("b", &["a", "b"])?;        // an element of an array, a slice or a Vec
//! sb.assert_in("ell", "hello")?;          // a substring of a string
//! sb.assert_in('e', &"hello".to_owned())?; // a character of a String
//! # Ok(())
//! # }
//! ```

/// A container that an item can be in: the elements of a slice, an array or a
/// [`Vec`], or the text of a [`str`] or a [`String`] (a substring, or a
/// character).
pub trait Contains<Item> {
    /// Whether `item` is in `self`.
    fn holds(&self, item: &Item) -> bool;
}

impl<T: PartialEq> Contains<T> for [T] {
    fn holds(&self, item: &T) -> bool {
        self.contains(item)
    }
}

impl<T: PartialEq, const N: usize> Contains<T> for [T; N] {
    fn holds(&self, item: &T) -> bool {
        self.as_slice().contains(item)
    }
}

impl<T: PartialEq> Contains<T> for Vec<T> {
    fn holds(&self, item: &T) -> bool {
        self.as_slice().contains(item)
    }
}

/// Makes a string type contain strings and characters.
macro_rules! text_contains {
    ($($container:ty),+) => {$(
        impl<'a> Contains<&'a str> for $container {
            fn holds(&self, item: &&'a str) -> bool {
                let text: &str = self.as_ref();
                text.contains(*item)
            }
        }

        impl Contains<String> for $container {
            fn holds(&self, item: &String) -> bool {
                let text: &str = self.as_ref();
                text.contains(item.as_str())
            }
        }

        impl<'a> Contains<&'a String> for $container {
            fn holds(&self, item: &&'a String) -> bool {
                let text: &str = self.as_ref();
                text.contains(item.as_str())
            }
        }

        impl Contains<char> for $container {
            fn holds(&self, item: &char) -> bool {
                let text: &str = self.as_ref();
                text.contains(*item)
            }
        }
    )+};
}

text_contains!(str, String);

#[cfg(test)]
mod tests {
    use crate::api::base_case::BaseCase;
    use crate::browser::config::BrowserConfig;
    use crate::error::SeleniumBaseError;

    fn case() -> BaseCase {
        BaseCase::without_session(BrowserConfig::default())
    }

    #[test]
    fn an_element_of_a_slice_an_array_or_a_vec_is_in_it() {
        let sb = case();
        sb.assert_in(2, &[1, 2, 3]).unwrap();
        sb.assert_in("b", &["a", "b"]).unwrap();
        sb.assert_in(3, &[1, 2, 3][..]).unwrap();
        let names = vec![String::from("a"), String::from("b")];
        sb.assert_in(String::from("b"), &names).unwrap();

        sb.assert_in(4, &[1, 2, 3]).unwrap_err();
        sb.assert_in(String::from("c"), &names).unwrap_err();
    }

    #[test]
    fn a_substring_is_in_a_string() {
        // Python: assert_in(framework, title.lower())
        let sb = case();
        let title = String::from("TodoMVC: Backbone.js");
        sb.assert_in("backbone", &title.to_lowercase()).unwrap();
        sb.assert_in("ell", "hello").unwrap();
        sb.assert_in(String::from("lo"), "hello").unwrap();
        let lo = String::from("lo");
        sb.assert_in(&lo, "hello").unwrap();
        sb.assert_in(lo, "hello").unwrap();
        sb.assert_in('e', "hello").unwrap();
        sb.assert_in("hello", "hello").unwrap();

        sb.assert_in("xyz", "hello").unwrap_err();
        sb.assert_in("backbone", &title).unwrap_err();
        sb.assert_in('z', &title).unwrap_err();
    }

    #[test]
    fn assert_not_in_is_the_other_way_round() {
        let sb = case();
        sb.assert_not_in("xyz", "hello").unwrap();
        sb.assert_not_in(4, &[1, 2, 3]).unwrap();
        sb.assert_not_in("ell", "hello").unwrap_err();
        sb.assert_not_in(2, &[1, 2, 3]).unwrap_err();
    }

    #[test]
    fn the_failure_names_the_item_and_the_container() {
        let sb = case();
        let error = sb.assert_in("xyz", "hello").unwrap_err();
        assert!(
            matches!(&error, SeleniumBaseError::AssertionFailed(message)
                if message.contains("xyz") && message.contains("hello")),
            "{error:?}"
        );
    }
}
