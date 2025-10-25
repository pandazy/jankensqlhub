# Motivation
After reviewing the current implementation of sql parameter types in sql, despite some convenience, it has a huge disadvantage which makes it potentially not scalable:
- parameter types needs to be duplicated everywhere together with parameters
- it also brings potential conflicts between different types on the same parameter in the same query

So we need a more production friendly design.

# Solution
- we only specify parameter names in sql
- we use a separate property "args" to define parameter specifications, take <repo>/test_json/def.json as example
    - it's a JSON object
    - Each key is the name of a parameter
    - each key define its specification in an object
        - type: the type of the parameter, still apply current types
        - range: an optional property specify the min/max allowed value of a `number`(integer/float) parameter
        - pattern: an optional property specify the allowed regex pattern of a `string` parameter
        - enum: an optional property specify the allowed values of a parameter, specified enum values must be the same type as the parameter's type