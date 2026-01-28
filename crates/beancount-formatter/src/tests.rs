use super::*;
use insta::assert_snapshot;

#[test]
fn test_basic_sorting() {
    let content = r#"2025-12-05 * "Transaction 3"
  Assets:Checking  -10 EUR

2025-12-01 * "Transaction 1"
  Assets:Checking  -20 EUR
"#;

    let sorted = sort(content);
    assert_snapshot!(sorted, @r#"
    2025-12-01 * "Transaction 1"
      Assets:Checking  -20 EUR

    2025-12-05 * "Transaction 3"
      Assets:Checking  -10 EUR
    "#);
}

#[test]
fn test_already_sorted() {
    let content = r#"2025-12-01 * "Transaction 1"
  Assets:Checking  -20 EUR

2025-12-05 * "Transaction 3"
  Assets:Checking  -10 EUR
"#;

    let sorted = sort(content);
    assert_snapshot!(sorted, @r#"
    2025-12-01 * "Transaction 1"
      Assets:Checking  -20 EUR

    2025-12-05 * "Transaction 3"
      Assets:Checking  -10 EUR
    "#);
}

#[test]
fn test_comments_as_boundaries() {
    let content = r#"; Header comment

2025-12-05 * "Transaction 3"
  Assets:Checking  -10 EUR

2025-12-01 * "Transaction 1"
  Assets:Checking  -20 EUR

; Middle comment

2025-12-03 * "Transaction 2"
  Assets:Checking  -30 EUR
"#;

    let sorted = sort(content);
    assert_snapshot!(sorted, @r#"
    ; Header comment

    2025-12-01 * "Transaction 1"
      Assets:Checking  -20 EUR

    2025-12-05 * "Transaction 3"
      Assets:Checking  -10 EUR

    ; Middle comment

    2025-12-03 * "Transaction 2"
      Assets:Checking  -30 EUR
    "#);
}

#[test]
fn test_stable_sort() {
    let content = r#"2025-12-01 * "Transaction A"
  Assets:A  -10 EUR

2025-12-01 * "Transaction B"
  Assets:B  -20 EUR

2025-12-01 * "Transaction C"
  Assets:C  -30 EUR
"#;

    let sorted = sort(content);
    assert_snapshot!(sorted, @r#"
    2025-12-01 * "Transaction A"
      Assets:A  -10 EUR

    2025-12-01 * "Transaction B"
      Assets:B  -20 EUR

    2025-12-01 * "Transaction C"
      Assets:C  -30 EUR
    "#);
}

#[test]
fn test_empty_file() {
    let content = "";
    let sorted = sort(content);
    assert_snapshot!(sorted, @"");
}

#[test]
fn test_only_comments() {
    let content = r#"; Comment 1
; Comment 2
"#;
    let sorted = sort(content);
    assert_snapshot!(sorted, @"
    ; Comment 1
    ; Comment 2
    ");
}

#[test]
fn test_balance_directives() {
    let content = r#"2025-12-05 balance Assets:Cash  100.00 EUR
2025-12-01 balance Assets:Cash  50.00 EUR
"#;

    let sorted = sort(content);
    assert_snapshot!(sorted, @"
    2025-12-01 balance Assets:Cash  50.00 EUR
    2025-12-05 balance Assets:Cash  100.00 EUR
    ");
}

#[test]
fn test_mixed_directive_types() {
    let content = r#"2025-12-05 * "Transaction"
  Assets:Cash  -10 EUR

2025-12-01 balance Assets:Cash  100.00 EUR

2025-12-03 price EUR  1.0 USD
"#;

    let sorted = sort(content);
    assert_snapshot!(sorted, @r#"
    2025-12-01 balance Assets:Cash  100.00 EUR

    2025-12-03 price EUR  1.0 USD

    2025-12-05 * "Transaction"
      Assets:Cash  -10 EUR
    "#);
}

#[test]
fn test_idempotent() {
    let content = r#"2025-12-05 * "Transaction 3"
  Assets:Checking  -10 EUR

2025-12-01 * "Transaction 1"
  Assets:Checking  -20 EUR
"#;

    let sorted1 = sort(content);
    let sorted2 = sort(&sorted1);
    assert_snapshot!(sorted2, @r#"
    2025-12-01 * "Transaction 1"
      Assets:Checking  -20 EUR

    2025-12-05 * "Transaction 3"
      Assets:Checking  -10 EUR
    "#);
}

#[test]
fn test_blank_lines_preserved() {
    let content = r#"2025-12-05 * "Transaction 3"
  Assets:Checking  -10 EUR


2025-12-01 * "Transaction 1"
  Assets:Checking  -20 EUR
"#;

    let sorted = sort(content);
    assert_snapshot!(sorted, @r#"
    2025-12-01 * "Transaction 1"
      Assets:Checking  -20 EUR


    2025-12-05 * "Transaction 3"
      Assets:Checking  -10 EUR
    "#);
}

#[test]
fn test_regression_1() {
    let content = r#"2025-12-29 * "a" "b"
    Assets:BIBEssen:Checking                         -17.99 EUR
    Expenses:Gifts                                    17.99 EUR

2025-12-29 * "a" "b"
    Assets:BIBEssen:Checking                         -10.54 EUR
    Expenses:Food:Groceries                           10.54 EUR

2025-12-30 * "a" "b"
    Assets:BIBEssen:Checking                         -99.00 EUR
    Expenses:Entertainment:Guitar                     99.00 EUR

2025-12-05 * "a" "b"
    Assets:BIBEssen:Checking                         -21.94 EUR
    Expenses:Car:Maintenance                          21.94 EUR
"#;

    let sorted = sort(content);
    assert_snapshot!(sorted, @r#"
    2025-12-05 * "a" "b"
        Assets:BIBEssen:Checking                         -21.94 EUR
        Expenses:Car:Maintenance                          21.94 EUR

    2025-12-29 * "a" "b"
        Assets:BIBEssen:Checking                         -17.99 EUR
        Expenses:Gifts                                    17.99 EUR

    2025-12-29 * "a" "b"
        Assets:BIBEssen:Checking                         -10.54 EUR
        Expenses:Food:Groceries                           10.54 EUR

    2025-12-30 * "a" "b"
        Assets:BIBEssen:Checking                         -99.00 EUR
        Expenses:Entertainment:Guitar                     99.00 EUR
    "#);
}

#[test]
fn test_regression_2() {
    let content = r#"2026-01-02 balance Assets:ScalableCapital:MsciWorldEM   426.1754 ~    0.01 IE00BKM4GZ66
2026-01-02 balance Assets:ScalableCapital:MsciWorld    2868.0171 ~    0.01 IE00BYX2JD69

2024-06-02 * "a" "b"
    Assets:Asset                                             -21.27 EUR
    Expenses:Gifts"#;

    let sorted = sort(content);
    assert_snapshot!(sorted, @r#"
    2024-06-02 * "a" "b"
        Assets:Asset                                             -21.27 EUR
        Expenses:Gifts

    2026-01-02 balance Assets:ScalableCapital:MsciWorldEM   426.1754 ~    0.01 IE00BKM4GZ66
    2026-01-02 balance Assets:ScalableCapital:MsciWorld    2868.0171 ~    0.01 IE00BYX2JD69
    "#);
}

// Tests for files that should be left exactly as-is (unchanged)

#[test]
fn test_unchanged_already_sorted_file() {
    let content = r#"2025-12-01 * "Transaction 1"
  Assets:Checking  -20 EUR

2025-12-02 * "Transaction 2"
  Assets:Checking  -30 EUR

2025-12-03 * "Transaction 3"
  Assets:Checking  -10 EUR
"#;

    let sorted = sort(content);
    assert_eq!(sorted, content, "Already sorted file should be unchanged");
}

#[test]
fn test_unchanged_only_comments() {
    let content = r#"; This is a comment
; Another comment
; More comments

; Even more comments
"#;

    let sorted = sort(content);
    assert_eq!(
        sorted, content,
        "File with only comments should be unchanged"
    );
}

#[test]
fn test_unchanged_only_non_dated_directives() {
    let content = r#"option "title" "My Ledger"
option "operating_currency" "USD"

plugin "beancount.plugins.auto_accounts"

include "accounts.beancount"
"#;

    let sorted = sort(content);
    assert_eq!(
        sorted, content,
        "File with only non-dated directives should be unchanged"
    );
}

#[test]
fn test_unchanged_empty_file() {
    let content = "";
    let sorted = sort(content);
    assert_eq!(sorted, content, "Empty file should be unchanged");
}

#[test]
fn test_unchanged_single_directive() {
    let content = r#"2025-12-01 * "Only transaction"
  Assets:Checking  -100 EUR
  Expenses:Food     100 EUR
"#;

    let sorted = sort(content);
    assert_eq!(
        sorted, content,
        "File with single directive should be unchanged"
    );
}

#[test]
fn test_unchanged_mixed_with_proper_order() {
    let content = r#"option "title" "My Ledger"

2020-01-01 open Assets:Checking

2025-12-01 * "Transaction 1"
  Assets:Checking  -20 EUR

2025-12-02 balance Assets:Checking  100.00 EUR

; End of file comment
"#;

    let sorted = sort(content);
    assert_eq!(
        sorted, content,
        "Properly ordered mixed file should be unchanged"
    );
}

#[test]
fn test_unchanged_with_blank_lines() {
    let content = r#"2025-12-01 * "Transaction 1"
  Assets:Checking  -20 EUR


2025-12-02 * "Transaction 2"
  Assets:Checking  -30 EUR
"#;

    let sorted = sort(content);
    assert_eq!(
        sorted, content,
        "Sorted file with blank lines should be unchanged"
    );
}

#[test]
fn test_unchanged_complex_sorted_file() {
    let content = r#"; Header
option "operating_currency" "EUR"

2020-01-01 open Assets:Checking EUR

2025-12-01 * "Transaction 1"
  Assets:Checking  -20 EUR

2025-12-02 * "Transaction 2"
  Assets:Checking  -30 EUR

; Middle section
2025-12-03 price EUR 1.1 USD

; End comment
"#;

    let sorted = sort(content);
    assert_eq!(sorted, content, "Complex sorted file should be unchanged");
}

#[test]
fn test_unchanged_single_import() {
    let content = r#"include "src/transactions.beancount""#;

    let sorted = sort(content);
    assert_eq!(sorted, content);
}

// Balance directive tests for blank line preservation

#[test]
fn test_balance_no_blanks() {
    // Two balances with no blank line between them, unsorted
    let content = r#"2026-01-02 balance Assets:B  200 USD
2026-01-01 balance Assets:A  100 USD
"#;

    let sorted = sort(content);
    assert_snapshot!(sorted, @r#"
    2026-01-01 balance Assets:A  100 USD
    2026-01-02 balance Assets:B  200 USD
    "#);
}

#[test]
fn test_balance_one_blank() {
    // Two balances with 1 blank line between them, unsorted
    let content = r#"2026-01-02 balance Assets:B  200 USD

2026-01-01 balance Assets:A  100 USD
"#;

    let sorted = sort(content);
    assert_snapshot!(sorted, @r"
    2026-01-01 balance Assets:A  100 USD

    2026-01-02 balance Assets:B  200 USD
    ");
}

#[test]
fn test_balance_mixed_blanks_three() {
    // Three balances: A-B have no blank, B-C have 1 blank, unsorted as C-B-A
    let content = r#"2026-01-03 balance Assets:C  300 USD

2026-01-02 balance Assets:B  200 USD
2026-01-01 balance Assets:A  100 USD
"#;

    let sorted = sort(content);
    // After sorting: A-B-C
    // A-B originally had no blank between them (they were consecutive in original as B-A)
    // B-C originally had 1 blank between them (they were consecutive in original as C-B)
    assert_snapshot!(sorted, @r"
    2026-01-01 balance Assets:A  100 USD
    2026-01-02 balance Assets:B  200 USD

    2026-01-03 balance Assets:C  300 USD
    ");
}

#[test]
fn test_balance_mixed_blanks_complex() {
    // Four balances with varying blank lines: D(1 blank)C(0 blanks)B(1 blank)A
    // After sorting should be A-B-C-D
    // Need to preserve: A-B had 1 blank, B-C had 0 blanks, C-D had 1 blank
    let content = r#"2026-01-04 balance Assets:D  400 USD

2026-01-03 balance Assets:C  300 USD
2026-01-02 balance Assets:B  200 USD

2026-01-01 balance Assets:A  100 USD
"#;

    let sorted = sort(content);
    assert_snapshot!(sorted, @r"
    2026-01-01 balance Assets:A  100 USD

    2026-01-02 balance Assets:B  200 USD
    2026-01-03 balance Assets:C  300 USD

    2026-01-04 balance Assets:D  400 USD
    ");
}

#[test]
fn test_balance_preserve_no_blanks_when_sorted() {
    // Already sorted balances with no blanks should stay unchanged
    let content = r#"2026-01-01 balance Assets:A  100 USD
2026-01-02 balance Assets:B  200 USD
2026-01-03 balance Assets:C  300 USD
"#;

    let sorted = sort(content);
    assert_eq!(
        sorted, content,
        "Already sorted balances with no blanks should be unchanged"
    );
}

#[test]
fn test_balance_preserve_mixed_when_sorted() {
    // Already sorted balances with mixed blank lines
    let content = r#"2026-01-01 balance Assets:A  100 USD
2026-01-02 balance Assets:B  200 USD

2026-01-03 balance Assets:C  300 USD
2026-01-04 balance Assets:D  400 USD
"#;

    let sorted = sort(content);
    assert_eq!(
        sorted, content,
        "Already sorted balances with mixed blanks should be unchanged"
    );
}

// Tests for non-dated directive boundaries

#[test]
fn test_option_boundary() {
    let content = r#"2025-12-05 * "Transaction 3"
  Assets:Checking  -10 EUR

option "operating_currency" "USD"

2025-12-01 * "Transaction 1"
  Assets:Checking  -20 EUR
"#;

    let sorted = sort(content);
    // Should NOT sort across the option boundary
    assert_snapshot!(sorted, @r#"
    2025-12-05 * "Transaction 3"
      Assets:Checking  -10 EUR

    option "operating_currency" "USD"

    2025-12-01 * "Transaction 1"
      Assets:Checking  -20 EUR
    "#);
}

#[test]
fn test_include_boundary() {
    let content = r#"2025-12-05 balance Assets:B  200 USD

include "accounts.beancount"

2025-12-01 balance Assets:A  100 USD
"#;

    let sorted = sort(content);
    assert_snapshot!(sorted, @r#"
    2025-12-05 balance Assets:B  200 USD

    include "accounts.beancount"

    2025-12-01 balance Assets:A  100 USD
    "#);
}

#[test]
fn test_plugin_boundary() {
    let content = r#"2025-12-05 * "Transaction 3"
  Assets:Checking  -10 EUR

plugin "beancount.plugins.auto_accounts"

2025-12-01 * "Transaction 1"
  Assets:Checking  -20 EUR
"#;

    let sorted = sort(content);
    assert_snapshot!(sorted, @r#"
    2025-12-05 * "Transaction 3"
      Assets:Checking  -10 EUR

    plugin "beancount.plugins.auto_accounts"

    2025-12-01 * "Transaction 1"
      Assets:Checking  -20 EUR
    "#);
}

#[test]
fn test_pushtag_poptag_boundary() {
    let content = r#"2025-12-05 * "Transaction 3"
  Assets:Checking  -10 EUR

pushtag #trip

2025-12-01 * "Transaction 1"
  Assets:Checking  -20 EUR

poptag #trip

2025-12-03 * "Transaction 2"
  Assets:Checking  -30 EUR
"#;

    let sorted = sort(content);
    // Each section should be separate
    assert_snapshot!(sorted, @r#"
    2025-12-05 * "Transaction 3"
      Assets:Checking  -10 EUR

    pushtag #trip

    2025-12-01 * "Transaction 1"
      Assets:Checking  -20 EUR

    poptag #trip

    2025-12-03 * "Transaction 2"
      Assets:Checking  -30 EUR
    "#);
}

#[test]
fn test_multiple_sections_with_options() {
    let content = r#"2025-12-05 * "Transaction 3"
  Assets:Checking  -10 EUR

2025-12-03 * "Transaction 2"
  Assets:Checking  -30 EUR

option "operating_currency" "USD"

2025-12-02 * "Transaction 1"
  Assets:Checking  -20 EUR

2025-12-01 * "Transaction 0"
  Assets:Checking  -5 EUR
"#;

    let sorted = sort(content);
    assert_snapshot!(sorted, @r#"
    2025-12-03 * "Transaction 2"
      Assets:Checking  -30 EUR

    2025-12-05 * "Transaction 3"
      Assets:Checking  -10 EUR

    option "operating_currency" "USD"

    2025-12-01 * "Transaction 0"
      Assets:Checking  -5 EUR

    2025-12-02 * "Transaction 1"
      Assets:Checking  -20 EUR
    "#);
}

#[test]
fn test_unchanged_open_and_pad_directives_same_date() {
    // When multiple directives share the same date, blank lines should be preserved
    let content = r#"2020-01-01 open Assets:BIBEssen:Checking EUR
2020-01-01 pad Assets:BIBEssen:Checking Equity:Opening-Balances

2020-01-01 open Assets:Revolut EUR
2020-01-01 open Expenses:Revolut:Fees EUR
"#;

    let sorted = sort(content);
    assert_eq!(
        sorted, content,
        "File with same-date open and pad directives should preserve blank lines"
    );
}
