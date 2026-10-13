#!/usr/bin/env python3
import argparse
import json
import sys
from pathlib import Path


class SchemaError(Exception):
    pass


class ValidationError(Exception):
    def __init__(self, path, message):
        super().__init__(f"{path}: {message}")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Validate AegisHV JSON or JSONL output against a local JSON schema."
    )
    parser.add_argument("--schema", required=True)
    inputs = parser.add_mutually_exclusive_group(required=True)
    inputs.add_argument("--jsonl")
    inputs.add_argument("--json")
    args = parser.parse_args()

    try:
        schema = load_json(Path(args.schema), "schema")
        if args.jsonl:
            count = validate_jsonl(schema, Path(args.jsonl))
            print(f"validated {count} events from {args.jsonl}")
        else:
            validate_json_document(schema, Path(args.json))
            print(f"validated JSON document from {args.json}")
    except (OSError, json.JSONDecodeError, SchemaError, ValidationError) as exc:
        print(str(exc), file=sys.stderr)
        return 1

    return 0


def load_json(path: Path, label: str):
    try:
        with path.open("r", encoding="utf-8") as f:
            return json.load(f)
    except json.JSONDecodeError as exc:
        raise json.JSONDecodeError(
            f"invalid {label} JSON in {path}: {exc.msg}",
            exc.doc,
            exc.pos,
        ) from exc


def validate_json_document(schema, path: Path):
    document = load_json(path, "input")
    validate_instance(document, schema, schema, "$")


def validate_jsonl(schema, path: Path) -> int:
    count = 0
    with path.open("r", encoding="utf-8") as f:
        for lineno, raw in enumerate(f, 1):
            line = raw.strip()
            if not line:
                continue
            count += 1
            try:
                document = json.loads(line)
            except json.JSONDecodeError as exc:
                raise ValidationError(
                    f"{path}:{lineno}",
                    f"invalid JSON: {exc.msg} at column {exc.colno}",
                ) from exc
            try:
                validate_instance(document, schema, schema, "$")
            except ValidationError as exc:
                raise ValidationError(f"{path}:{lineno}", str(exc)) from exc

    if count == 0:
        raise ValidationError(str(path), "no JSON documents")
    return count


def validate_instance(value, schema, root_schema, path: str):
    if not isinstance(schema, dict):
        raise SchemaError(f"{path}: schema node is not an object")

    if "$ref" in schema:
        validate_instance(value, resolve_ref(schema["$ref"], root_schema), root_schema, path)
        return

    if "const" in schema and value != schema["const"]:
        raise ValidationError(path, f"expected const {schema['const']!r}, got {value!r}")

    if "enum" in schema and value not in schema["enum"]:
        raise ValidationError(path, f"value {value!r} is not in enum {schema['enum']!r}")

    if "type" in schema:
        expected_types = schema["type"]
        if isinstance(expected_types, str):
            expected_types = [expected_types]
        if not any(type_matches(value, expected) for expected in expected_types):
            raise ValidationError(
                path,
                f"expected type {expected_types!r}, got {json_type_name(value)}",
            )

    if isinstance(value, dict):
        validate_object(value, schema, root_schema, path)
    elif isinstance(value, list):
        validate_array(value, schema, root_schema, path)

    validate_bounds(value, schema, path)


def validate_object(value, schema, root_schema, path: str):
    required = schema.get("required", [])
    for key in required:
        if key not in value:
            raise ValidationError(join_path(path, key), "required property is missing")

    properties = schema.get("properties", {})
    if schema.get("additionalProperties") is False:
        for key in sorted(set(value) - set(properties)):
            raise ValidationError(join_path(path, key), "additional property is not allowed")

    for key, subschema in properties.items():
        if key in value:
            validate_instance(value[key], subschema, root_schema, join_path(path, key))


def validate_array(value, schema, root_schema, path: str):
    item_schema = schema.get("items")
    if item_schema is None:
        return
    for idx, item in enumerate(value):
        validate_instance(item, item_schema, root_schema, f"{path}[{idx}]")


def validate_bounds(value, schema, path: str):
    if isinstance(value, str) and "minLength" in schema and len(value) < schema["minLength"]:
        raise ValidationError(path, f"string length is below {schema['minLength']}")

    if is_json_number(value):
        if "minimum" in schema and value < schema["minimum"]:
            raise ValidationError(path, f"value is below minimum {schema['minimum']}")
        if "maximum" in schema and value > schema["maximum"]:
            raise ValidationError(path, f"value is above maximum {schema['maximum']}")


def resolve_ref(ref: str, root_schema):
    if not ref.startswith("#/"):
        raise SchemaError(f"unsupported schema ref {ref!r}")
    node = root_schema
    for raw_part in ref[2:].split("/"):
        part = raw_part.replace("~1", "/").replace("~0", "~")
        if not isinstance(node, dict) or part not in node:
            raise SchemaError(f"schema ref {ref!r} does not resolve")
        node = node[part]
    return node


def type_matches(value, expected: str) -> bool:
    if expected == "null":
        return value is None
    if expected == "boolean":
        return type(value) is bool
    if expected == "integer":
        return type(value) is int
    if expected == "number":
        return is_json_number(value)
    if expected == "string":
        return isinstance(value, str)
    if expected == "object":
        return isinstance(value, dict)
    if expected == "array":
        return isinstance(value, list)
    raise SchemaError(f"unsupported schema type {expected!r}")


def is_json_number(value) -> bool:
    return (type(value) is int) or (type(value) is float)


def json_type_name(value) -> str:
    if value is None:
        return "null"
    if type(value) is bool:
        return "boolean"
    if type(value) is int:
        return "integer"
    if type(value) is float:
        return "number"
    if isinstance(value, str):
        return "string"
    if isinstance(value, dict):
        return "object"
    if isinstance(value, list):
        return "array"
    return type(value).__name__


def join_path(path: str, key: str) -> str:
    return f"{path}.{key}"


if __name__ == "__main__":
    raise SystemExit(main())
