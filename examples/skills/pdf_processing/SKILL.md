---
name: pdf_processing
description: Process and manipulate PDF documents, including text extraction, form filling, and metadata editing
category: document
tags: [pdf, document, forms]
version: "1.0.0"
---

# PDF Processing Skill

This skill provides comprehensive capabilities for working with PDF documents.

## When to Use

Use this skill when you need to:
- Extract text or data from PDF files
- Fill out PDF forms
- Edit PDF metadata
- Analyze PDF structure

## Text Extraction

For basic text extraction:
1. Provide the PDF file path
2. Specify extraction options (plain text, structured data)

## Form Filling

For form-specific operations, see @include: forms.md

## Code Tools

This skill includes the following executable tools:
- @code: python extract_fields.py - Extract form fields from a PDF

## Example Usage

To extract text from a PDF:
```
Use the pdf_processing skill to extract all text from the document.
```

To fill a form:
```
Use the pdf_processing skill and load forms.md for detailed instructions.
```
