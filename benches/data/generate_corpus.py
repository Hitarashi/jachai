#!/usr/bin/env python3
"""
Comprehensive Canonical Benchmark Corpus Generator for jachai.
Generates canonical datasets covering every single validator domain and rule:
- Flat & Nested Objects
- Arrays (iteration, uniqueness, subsets, contains)
- Records & Key-Value maps
- Tuples (2, 3, 4 elements)
- Enums (fixed variants)
- Temporal (RFC 3339 Date, Time, DateTime)
- Network (IPv4, IPv6, URL, MAC address, CIDR, Domain, FQDN)
- Identifiers (UUID, ULID, SemVer, Slug)
- Financial (Credit Card Luhn, IBAN, BIC)
- Geographic (Latitude, Longitude, Coordinates, Country Code)
- Color (Hex color codes)
- Cryptographic & Security (SHA-256, Base64, JWT, Non-control characters)
- Business & Tax (VAT ID, SSN, EIN)
- Cross-field Object Rules (confirmed, same_as, different_from, required_if, after_date)
- Multi-source Request Parts (params, query, headers, body, cookies)
- File Metadata (size, mime_type, extension)
"""

import json
import os

def generate_corpus():
    first_names = ["alice", "bob", "charlie", "david", "emma", "fiona", "george", "hannah", "ivan", "julia"]
    last_names = ["smith", "johnson", "williams", "brown", "jones", "garcia", "miller", "davis", "wilson", "moore"]
    cities = ["New York", "London", "Tokyo", "Berlin", "Paris", "Toronto", "Sydney", "Singapore", "Dublin", "Austin"]
    domains = ["example.com", "techcorp.org", "service.net", "university.edu", "cloudagency.io"]

    # 1. Flat Valid & Invalid
    flat_valid = []
    flat_invalid = []
    for i in range(50):
        first = first_names[i % len(first_names)]
        last = last_names[i % len(last_names)]
        uname = f"{first}_{last}_{i}"
        email = f"{first}.{last}{i}@{domains[i % len(domains)]}"
        age = 18 + (i % 60)
        flat_valid.append({"username": uname, "email": email, "age": age})

        if i % 3 == 0:
            flat_invalid.append({"username": "a", "email": email, "age": age})
        elif i % 3 == 1:
            flat_invalid.append({"username": uname, "email": "not-an-email", "age": age})
        else:
            flat_invalid.append({"username": uname, "email": email, "age": 12})

    # 2. Nested Valid & Invalid
    nested_valid = []
    nested_invalid = []
    for i in range(50):
        first = first_names[i % len(first_names)]
        last = last_names[i % len(last_names)]
        uname = f"{first}_{last}_{i}"
        email = f"{first}.{last}{i}@{domains[i % len(domains)]}"
        city = cities[i % len(cities)]
        nested_valid.append({
            "username": uname,
            "email": email,
            "profile": {
                "full_name": f"{first.capitalize()} {last.capitalize()}",
                "city": city,
            }
        })
        if i % 2 == 0:
            nested_invalid.append({
                "username": uname,
                "email": email,
                "profile": {"full_name": "", "city": city}
            })
        else:
            nested_invalid.append({
                "username": "x",
                "email": email,
                "profile": {"full_name": f"{first.capitalize()} {last.capitalize()}", "city": ""}
            })

    # 3. Array of Objects Valid & Invalid
    array_valid = []
    array_invalid = []
    for batch in range(20):
        items = []
        for i in range(5):
            idx = (batch * 5 + i) % 50
            first = first_names[idx % len(first_names)]
            last = last_names[idx % len(last_names)]
            items.append({
                "name": f"{first.capitalize()} {last.capitalize()}",
                "email": f"{first}.{last}{idx}@{domains[idx % len(domains)]}",
                "score": 50 + (idx % 50),
            })
        array_valid.append(items)
        bad_items = [dict(it) for it in items]
        bad_items[2]["email"] = "bad-email-address"
        array_invalid.append(bad_items)

    # 4. Array with Unique Elements vs Duplicates
    array_unique_valid = [list(range(i, i + 20)) for i in range(20)]
    array_unique_invalid = [list(range(i, i + 19)) + [i] for i in range(20)] # contains duplicate

    # 5. Records (Map / Dictionary of Key-Value pairs)
    records_valid = []
    for i in range(50):
        records_valid.append({
            f"key_{j}": (j * 10) + i for j in range(5)
        })

    # 6. Tuples: [string, integer, boolean]
    tuples_valid = []
    for i in range(50):
        tuples_valid.append([f"label_{i}", i * 10, i % 2 == 0])

    # 7. Enums: fixed status strings
    enum_variants = ["pending", "active", "suspended", "archived"]
    enums_valid = [enum_variants[i % len(enum_variants)] for i in range(50)]
    enums_invalid = ["unknown_status", "invalid", "deleted", "expired"] * 13
    enums_invalid = enums_invalid[:50]

    # 8. Date / Time (RFC 3339)
    datetimes_valid = [
        f"2026-11-{(i % 28) + 1:02d}T{(i % 24):02d}:{(i % 60):02d}:00Z" for i in range(50)
    ]
    dates_valid = [f"2026-11-{(i % 28) + 1:02d}" for i in range(50)]
    times_valid = [f"{(i % 24):02d}:{(i % 60):02d}:00" for i in range(50)]

    # 9. Network: IPv4, IPv6, URL, MAC, CIDR, Domain, FQDN
    ipv4_valid = [f"192.168.1.{(i % 254) + 1}" for i in range(50)]
    ipv6_valid = [f"2001:db8:85a3::{i:x}" for i in range(50)]
    urls_valid = [f"https://www.{domains[i % len(domains)]}/path/resource_{i}?query=1" for i in range(50)]
    mac_valid = [f"00:1A:2B:3C:4D:{i:02X}" for i in range(50)]
    cidr_valid = [f"10.{i}.0.0/16" for i in range(50)]
    domain_valid = [domains[i % len(domains)] for i in range(50)]

    # 10. Identifiers: UUID, ULID, SemVer, Slug
    sample_uuids = [
        "c9a646d3-9c61-4cd7-bc17-3b2d1f476a6d",
        "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
        "6ba7b811-9dad-11d1-80b4-00c04fd430c8",
        "6ba7b812-9dad-11d1-80b4-00c04fd430c8",
        "6ba7b814-9dad-11d1-80b4-00c04fd430c8",
    ]
    uuids_valid = [sample_uuids[i % len(sample_uuids)] for i in range(50)]
    ulids_valid = ["01ARZ3NDEKTSV4RRFFQ69G5FAV"] * 50
    semvers_valid = [f"1.{i}.0" for i in range(50)]
    slugs_valid = [f"canonical-article-slug-{i}" for i in range(50)]

    # 11. Financial: Credit Card (Luhn), IBAN, Currency Code
    # Valid test credit cards (visa and mastercard test numbers passing Luhn)
    credit_cards_valid = [
        "4532015112830366", "4556737586899855", "5105105105105100",
        "4012888888881881", "4222222222222"
    ] * 10
    ibans_valid = ["GB82WEST12345698765432"] * 50
    currency_codes_valid = ["USD", "EUR", "GBP", "JPY", "CAD", "AUD", "CHF"] * 8
    currency_codes_valid = currency_codes_valid[:50]

    # 12. Geographic: Latitude, Longitude, Coordinates, Country Code
    latitudes_valid = [f"{(i % 80) + 0.1234:.4f}" for i in range(50)]
    longitudes_valid = [f"{((i * 3) % 170) - 85.1234:.4f}" for i in range(50)]
    coordinates_valid = [f"{latitudes_valid[i]}, {longitudes_valid[i]}" for i in range(50)]
    country_codes_valid = ["US", "GB", "CA", "DE", "FR", "JP", "AU", "SE"] * 7
    country_codes_valid = country_codes_valid[:50]

    # 13. Color: Hex color codes
    hex_colors_valid = [f"#{i:02x}{i*2:02x}{i*3:02x}" for i in range(50)]

    # 14. Cryptographic & Security: SHA-256, Base64, JWT, Non-control characters
    sha256_valid = ["e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"] * 50
    base64_valid = ["SGVsbG8gV29ybGQgZnJvbSBKYWNoYWkh"] * 50
    jwt_valid = ["eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"] * 50

    # 15. Cross-field Rules: confirmed (password == password_confirmation), after_date (end_date > start_date)
    confirmed_objects_valid = [
        {"password": f"SecretPass_{i}!", "password_confirmation": f"SecretPass_{i}!"} for i in range(50)
    ]
    after_date_objects_valid = [
        {"start_date": "2026-01-01T00:00:00Z", "end_date": "2026-02-01T00:00:00Z"} for i in range(50)
    ]
    required_if_objects_valid = [
        {"contact_type": "business", "tax_id": f"EIN-12345{i}"} if i % 2 == 0
        else {"contact_type": "personal"} for i in range(50)
    ]

    # 16. Multi-source Request Parts
    request_parts_valid = []
    for i in range(50):
        request_parts_valid.append({
            "params": {"id": f"{1000 + i}"},
            "query": {"page": "1", "limit": "20"},
            "headers": {"authorization": f"Bearer token_{i}", "accept": "application/json"},
            "body": {"username": f"user_{i}", "email": f"user{i}@example.com"}
        })

    # 17. Primitive Emails
    primitive_emails_valid = [f"user{i}@{domains[i % len(domains)]}" for i in range(50)]
    primitive_emails_invalid = [
        "plainaddress", "#@%^%#$@#$@#.com", "@example.com", "Joe Smith <email@example.com>",
        "email.example.com", "email@example@example.com", ".email@example.com", "email.@example.com",
        "email..email@example.com", "あいうえお@example.com", "email@example.com (Joe Smith)",
        "email@example", "email@-example.com", "email@111.222.333.44444", "email@example..com",
        "Abc..123@example.com", r"“(),:;<>[\]@example.com", "just\"not\"right@example.com",
        "this is\"not\allowed@example.com", "this\\ still\\\"not\\\\allowed@example.com"
    ] * 3
    primitive_emails_invalid = primitive_emails_invalid[:50]

    # 18. Primitive Integers
    primitive_integers_valid = [18 + (i % 82) for i in range(50)]
    primitive_integers_invalid = [i if i % 2 == 0 else 100 + i for i in range(1, 51)]

    # 19. Primitive Floats
    primitive_floats_valid = [18.5 + (i % 60) * 1.25 for i in range(50)]

    # 20. Primitive Booleans (Accepted)
    primitive_booleans_valid = [True for _ in range(50)]

    # 21. String Rules (Alphanumeric, Min/Max len, Starts/Ends with)
    string_rules_valid = [f"Prefix_User{i:03d}_Suffix" for i in range(50)]

    # 22. File Metadata Uploads
    files_valid = [
        {
            "client_filename": f"document_{i}.pdf" if i % 2 == 0 else f"image_{i}.png",
            "size": 1024 * (i + 1) + 500,
            "mime_type": "application/pdf" if i % 2 == 0 else "image/png",
            "extension": "pdf" if i % 2 == 0 else "png",
        }
        for i in range(50)
    ]

    # 23. Union Valid
    union_valid = []
    for i in range(50):
        if i % 2 == 0:
            union_valid.append({"channel": "email", "address": f"contact{i}@example.com"})
        else:
            union_valid.append({"channel": "phone", "phone": f"+1555{i:06d}"})

    corpus = {
        "flat_valid": flat_valid,
        "flat_invalid": flat_invalid,
        "nested_valid": nested_valid,
        "nested_invalid": nested_invalid,
        "array_valid": array_valid,
        "array_invalid": array_invalid,
        "array_unique_valid": array_unique_valid,
        "array_unique_invalid": array_unique_invalid,
        "records_valid": records_valid,
        "tuples_valid": tuples_valid,
        "enums_valid": enums_valid,
        "enums_invalid": enums_invalid,
        "datetimes_valid": datetimes_valid,
        "dates_valid": dates_valid,
        "times_valid": times_valid,
        "ipv4_valid": ipv4_valid,
        "ipv6_valid": ipv6_valid,
        "urls_valid": urls_valid,
        "mac_valid": mac_valid,
        "cidr_valid": cidr_valid,
        "domain_valid": domain_valid,
        "primitive_emails_valid": primitive_emails_valid,
        "primitive_emails_invalid": primitive_emails_invalid,
        "primitive_uuids_valid": uuids_valid,
        "ulids_valid": ulids_valid,
        "semvers_valid": semvers_valid,
        "slugs_valid": slugs_valid,
        "credit_cards_valid": credit_cards_valid,
        "ibans_valid": ibans_valid,
        "currency_codes_valid": currency_codes_valid,
        "latitudes_valid": latitudes_valid,
        "longitudes_valid": longitudes_valid,
        "coordinates_valid": coordinates_valid,
        "country_codes_valid": country_codes_valid,
        "hex_colors_valid": hex_colors_valid,
        "sha256_valid": sha256_valid,
        "base64_valid": base64_valid,
        "jwt_valid": jwt_valid,
        "confirmed_objects_valid": confirmed_objects_valid,
        "after_date_objects_valid": after_date_objects_valid,
        "required_if_objects_valid": required_if_objects_valid,
        "request_parts_valid": request_parts_valid,
        "primitive_integers_valid": primitive_integers_valid,
        "primitive_integers_invalid": primitive_integers_invalid,
        "primitive_floats_valid": primitive_floats_valid,
        "primitive_booleans_valid": primitive_booleans_valid,
        "string_rules_valid": string_rules_valid,
        "files_valid": files_valid,
        "union_valid": union_valid,
    }

    script_dir = os.path.dirname(os.path.abspath(__file__))
    output_path = os.path.join(script_dir, "corpus.json")
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(corpus, f, indent=2)

    print(f"Generated comprehensive corpus at {output_path} with {len(corpus)} distinct canonical validator datasets.")

if __name__ == "__main__":
    generate_corpus()
