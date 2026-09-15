# Install with pip install firecrawl-py
from firecrawl import FirecrawlApp
from pydantic import BaseModel

import os
import sys
import json

url = sys.argv[1]
conf = sys.argv[2]

app = FirecrawlApp(api_key=os.environ['FIRECRAWL_API_KEY'])

class Event(BaseModel):
    event: str
    date: str

class ExtractSchema(BaseModel):
    important_dates: list[Event]

data = app.extract([f"{url}/*"], {
    'prompt': f"Extract all important dates related to the {conf} conference, including the event name and corresponding date. Also include all date ranges.",
    'schema': ExtractSchema.model_json_schema()
})
print(json.dumps(data, indent=2))
