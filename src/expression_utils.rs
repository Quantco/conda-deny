use anyhow::{Context, Result};
use spdx::{Expression, LicenseItem, LicenseReq, ParseMode};

pub fn extract_license_ids(expression: &Expression) -> Vec<String> {
    expression
        .requirements()
        .map(|req| match &req.req.license {
            LicenseItem::Spdx { id, .. } => id.name.to_string(),
            LicenseItem::Other { lic_ref, .. } => lic_ref.clone(),
        })
        .collect()
}

fn check_license_req_safety(license_req: &LicenseReq, safe_licenses: &[Expression]) -> bool {
    let safe_license_ids: Vec<String> =
        safe_licenses.iter().flat_map(extract_license_ids).collect();

    match &license_req.license {
        LicenseItem::Spdx { id, .. } => safe_license_ids.contains(&id.name.to_string()),
        LicenseItem::Other { lic_ref, .. } => safe_license_ids.contains(lic_ref),
    }
}

pub fn check_expression_safety(expression: &Expression, safe_licenses: &[Expression]) -> bool {
    expression.evaluate(|req| check_license_req_safety(req, safe_licenses))
}

pub fn parse_expression(expression_str: &str) -> Result<Expression> {
    let parse_mode = ParseMode {
        allow_imprecise_license_names: false,
        allow_slash_as_or_operator: false,
        allow_lower_case_operators: true,
        allow_postfix_plus_on_gpl: false,
    };

    Expression::parse_mode(expression_str, parse_mode)
        .with_context(|| format!("Failed to parse expression: '{}'", expression_str))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::expression_utils::parse_expression;

    #[test]
    fn test_extract_license_ids() {
        let expression = parse_expression("MIT OR GPL-3.0-or-later").unwrap();
        let license_ids = super::extract_license_ids(&expression);

        assert_eq!(license_ids, vec!["MIT".to_string(), "GPL-3.0".to_string()]);
    }

    #[test]
    fn test_check_license_req_safety() {
        let expression = parse_expression("MIT").unwrap();
        let safe_licenses = vec![Expression::parse("MIT").unwrap()];

        for req in expression.requirements() {
            assert!(super::check_license_req_safety(&req.req, &safe_licenses));
        }
    }

    #[test]
    fn test_check_expression_safety() {
        let safe_licenses = vec![
            Expression::parse("MIT").unwrap(),
            Expression::parse("BSD-3-Clause").unwrap(),
        ];

        let expression = parse_expression("MIT").unwrap();
        let or_expression = parse_expression("MIT OR BSD-2-Clause").unwrap();
        let valid_and_expression = parse_expression("MIT AND BSD-3-Clause").unwrap();
        let invalid_and_expression = parse_expression("MIT AND BSD-2-Clause").unwrap();

        assert!(super::check_expression_safety(
            &or_expression,
            &safe_licenses
        ));
        assert!(super::check_expression_safety(
            &valid_and_expression,
            &safe_licenses
        ));
        assert!(super::check_expression_safety(&expression, &safe_licenses));
        assert!(!super::check_expression_safety(
            &invalid_and_expression,
            &safe_licenses
        ));
    }

    #[test]
    fn test_parse_expression_lowercase_and_or() {
        let expression_lower_and = super::parse_expression("MIT and PSF-2.0").unwrap();
        let expression_higher_and = super::parse_expression("MIT AND PSF-2.0").unwrap();
        let expression_lower_or = super::parse_expression("MIT or PSF-2.0").unwrap();
        let expression_higher_or = super::parse_expression("MIT OR PSF-2.0").unwrap();

        assert_eq!(
            expression_lower_and.to_string(),
            "MIT and PSF-2.0".to_string()
        );
        assert_eq!(
            expression_higher_and.to_string(),
            "MIT AND PSF-2.0".to_string()
        );
        assert_eq!(
            expression_lower_or.to_string(),
            "MIT or PSF-2.0".to_string()
        );
        assert_eq!(
            expression_higher_or.to_string(),
            "MIT OR PSF-2.0".to_string()
        );
    }
}
