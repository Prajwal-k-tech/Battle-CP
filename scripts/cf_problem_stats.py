#!/usr/bin/env python3
"""
Query Codeforces API problemset to count problems at each rating level.
Also checks what ratings are actually available.
"""
import asyncio
import aiohttp
import json
from collections import Counter

async def main():
    async with aiohttp.ClientSession() as session:
        print("Fetching CF problemset.problems (all problems)...")
        async with session.get("https://codeforces.com/api/problemset.problems") as resp:
            data = await resp.json()
        
        if data["status"] != "OK":
            print(f"Error: {data}")
            return
        
        problems = data["result"]["problems"]
        print(f"Total problems: {len(problems)}")
        
        # Count by rating
        rating_counts = Counter()
        no_rating = 0
        for p in problems:
            r = p.get("rating")
            if r is not None:
                rating_counts[r] += 1
            else:
                no_rating += 1
        
        print(f"\nProblems WITHOUT rating: {no_rating}")
        print(f"Problems WITH rating: {sum(rating_counts.values())}")
        
        print(f"\n{'Rating':<10} {'Count':<10} {'Bar'}")
        print("-" * 60)
        for rating in sorted(rating_counts.keys()):
            count = rating_counts[rating]
            bar = "█" * min(count // 20, 50)
            print(f"{rating:<10} {count:<10} {bar}")
        
        # Also count by contest type using problem index
        print(f"\n\n--- Division analysis at rating 800 ---")
        r800 = [p for p in problems if p.get("rating") == 800]
        index_counts = Counter(p["index"] for p in r800)
        print(f"Total 800-rated: {len(r800)}")
        for idx in sorted(index_counts.keys()):
            print(f"  Index {idx}: {index_counts[idx]}")
        
        # Show by tags for 800
        tag_counts = Counter()
        for p in r800:
            for t in p.get("tags", []):
                tag_counts[t] += 1
        print(f"\nTop tags at 800:")
        for tag, count in tag_counts.most_common(10):
            print(f"  {tag}: {count}")

asyncio.run(main())
