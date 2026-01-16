---
name: web_scraping
description: Extract data from web pages, including HTML parsing, API interaction, and data export
category: web
tags: [scraping, http, api, data]
version: "1.0.0"
---

# Web Scraping Skill

This skill enables web scraping and data extraction capabilities.

## When to Use

Use this skill when you need to:
- Scrape data from websites
- Interact with REST APIs
- Parse HTML content
- Export scraped data to various formats

## Scraping Methods

### Static HTML Scraping
For static websites, use CSS selectors to extract data.

### Dynamic Content
For JavaScript-heavy sites, use browser automation.

### API Access
For structured data access, prefer REST or GraphQL APIs.

## Code Tools

This skill includes:
- @code: python scraper.py --input {input} - Generic web scraper
- @code: bash fetch.sh {url} - Simple HTTP fetcher

## Best Practices

1. **Check robots.txt**: Always respect site scraping policies
2. **Rate limiting**: Add delays between requests to avoid overwhelming servers
3. **User-Agent**: Identify your bot properly
4. **Error handling**: Gracefully handle network errors and timeouts

## Example

To scrape a webpage:
```
Use the web_scraping skill to extract product information from https://example.com/products
```
