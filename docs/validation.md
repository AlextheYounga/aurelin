# Structured validation issues

`aurelin_framework::validation::issues` converts `validator` 0.20 errors into
serializable issues. Each issue contains a concrete dot-separated `field`, the
validator rule `code`, a default English `message`, named `arguments`, and an
optional `bag`. Nested validator structs and lists include their concrete list
indices. Wildcard rule matching is not provided by this conversion.

Rule codes are public identifiers: applications should keep codes for custom
rules stable. The current adapter uses validator's rule codes unchanged; a
future dependency upgrade must preserve these identifiers or be treated as a
contract change. Only numeric `min`, `max`, and `equal` parameters from length
and range rules are copied. Rejected values and other validator parameters are
not copied. Custom validation messages must not contain sensitive values.

The English fallback is deliberately independent of any locale, catalogue, or
translator. A future response renderer can use the code and safe arguments to
choose another message without changing validation rules. The adapter alone
does not install request extractors, HTTP error responses, redirects, session
flashing, or authorization ordering.
