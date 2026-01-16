#!/usr/bin/env python3
"""
Text Analyzer
Performs sentiment analysis, entity extraction, and keyword analysis
"""

import json
import re
import sys


def analyze_sentiment(text):
    """Analyze sentiment of text."""
    # Simple rule-based sentiment analysis
    positive_words = ['good', 'great', 'excellent', 'amazing', 'wonderful', 'exceeded', 'love']
    negative_words = ['bad', 'terrible', 'awful', 'hate', 'disappointed', 'poor', 'worst']

    text_lower = text.lower()
    pos_count = sum(1 for word in positive_words if word in text_lower)
    neg_count = sum(1 for word in negative_words if word in text_lower)

    if pos_count > neg_count:
        return {"sentiment": "positive", "confidence": min(0.9, 0.5 + pos_count * 0.1)}
    elif neg_count > pos_count:
        return {"sentiment": "negative", "confidence": min(0.9, 0.5 + neg_count * 0.1)}
    else:
        return {"sentiment": "neutral", "confidence": 0.5}


def extract_entities(text):
    """Extract named entities from text."""
    entities = {
        "organizations": [],
        "people": [],
        "locations": []
    }

    # Simple pattern matching (in production, use spaCy or similar)
    # Capitalized words that might be entities
    words = re.findall(r'\b[A-Z][a-z]+\b', text)
    entities["potential"] = list(set(words))

    return entities


def extract_keywords(text):
    """Extract important keywords from text."""
    # Remove common words and get significant terms
    words = re.findall(r'\b[a-z]{4,}\b', text.lower())
    word_freq = {}
    for word in words:
        word_freq[word] = word_freq.get(word, 0) + 1

    # Get top 5 keywords
    sorted_words = sorted(word_freq.items(), key=lambda x: x[1], reverse=True)[:5]
    return {"keywords": [w[0] for w in sorted_words]}


def analyze_text(text, analysis_type="all"):
    """Perform comprehensive text analysis."""
    result = {
        "text": text[:100] + "..." if len(text) > 100 else text,
        "analyses": []
    }

    if analysis_type in ["all", "sentiment"]:
        result["analyses.append("sentiment")
        result["sentiment"] = analyze_sentiment(text)

    if analysis_type in ["all", "entities"]:
        result["analyses"].append("entities")
        result["entities"] = extract_entities(text)

    if analysis_type in ["all", "keywords"]:
        result["analyses"].append("keywords")
        result["keywords"] = extract_keywords(text)

    return result


if __name__ == "__main__":
    if len(sys.argv) > 2 and sys.argv[1] == "--json":
        args = json.loads(sys.argv[2])
        text = args.get("text", "")
        analysis_type = args.get("type", "all")
        result = analyze_text(text, analysis_type)
        print(json.dumps(result, indent=2))
    else:
        print(json.dumps({"error": "Use --json with JSON arguments"}))
