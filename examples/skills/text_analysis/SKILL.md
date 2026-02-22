---
name: text_analysis
description: Analyze text for sentiment, entities, keywords, and linguistic patterns
category: nlp
tags: [text, analysis, nlp, sentiment]
version: "1.0.0"
---

# Text Analysis Skill

This skill provides natural language processing capabilities for text analysis.

## When to Use

Use this skill when you need to:
- Analyze text sentiment (positive/negative/neutral)
- Extract named entities (people, places, organizations)
- Identify keywords and phrases
- Perform text classification
- Summarize long documents

## Analysis Types

### Sentiment Analysis
Determines the emotional tone of text:
- **Positive**: Expresses satisfaction, happiness, approval
- **Negative**: Expresses dissatisfaction, sadness, disapproval
- **Neutral**: Factual or informational content

### Entity Extraction
Identifies and categorizes:
- **People**: Person names
- **Places**: Locations, addresses
- **Organizations**: Companies, institutions
- **Dates**: Temporal expressions
- **Numbers**: Quantities, measurements

### Keyword Analysis
Extracts important terms and phrases:
- TF-IDF scoring
- Phrase frequency
- Collocation detection

## Code Tools

@code: python analyze.py --input {input} - Perform various text analyses

## Example Usage

```
Analyze the sentiment of this customer review: "The product exceeded my expectations!"
Result: Positive sentiment detected (confidence: 0.95)
```

```
Extract entities from: "Apple Inc. was founded by Steve Jobs in Cupertino, California."
Entities:
- Organization: Apple Inc.
- Person: Steve Jobs
- Location: Cupertino, California
```
