# PDF Form Filling Guide

This guide explains how to fill out PDF forms using the pdf_processing skill.

## Form Field Types

PDF forms support various field types:
- **Text fields**: Single-line or multi-line text input
- **Checkboxes**: Boolean selection
- **Radio buttons**: Single choice from options
- **Dropdown lists**: Select from predefined options
- **Signature fields**: Digital signature areas

## Filling Process

1. **Load the PDF**: Read the PDF file to identify form fields
2. **Extract field names**: Get the names and types of all form fields
3. **Provide values**: Map field names to their values
4. **Generate filled PDF**: Create a new PDF with filled values

## Example

```python
# Use extract_fields.py to get form structure
python3 extract_fields.py form.pdf

# Output shows:
# Field 1: name (Text)
# Field 2: email (Text)
# Field 3: agree (Checkbox)
```
