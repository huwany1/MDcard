# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Independently verify release links captured from the deployed page.

No GitHub credentials, netrc, or saved browser cookies are used. Bodies are
streamed into SHA256 rather than retained as another full installation archive.
"""
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
from email.message import Message
from urllib.parse import urlparse, unquote
import argparse
import hashlib
import json
import platform
import sys
import time

import requests

SITE = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--page-links', type=Path, required=True)
parser.add_argument('--platform', choices=['windows', 'linux'], action='append')
parser.add_argument('--out', type=Path, default=SITE / 'evidence/master/public-downloads.json')
args = parser.parse_args()
captured = json.loads(args.page_links.read_text(encoding='utf-8'))
releases = json.loads((SITE / 'src/data/releases.json').read_text(encoding='utf-8'))['releases']
selected = [r for r in releases if not args.platform or r['platform'] in args.platform]


def verify(item):
    started = time.time()
    result = {key: item[key] for key in ['platform', 'build_id', 'filename', 'bytes', 'sha256']}
    result['attempts'] = []
    link = next((x['url'] for x in captured['links'] if x['platform'] == item['platform']), None)
    result['url'] = link
    if not link or link != item['download_url'] or urlparse(link).scheme != 'https':
        return {**result, 'passed': False, 'error': 'Published link does not match release data'}
    for attempt in range(1, 4):
        session = requests.Session()
        session.trust_env = False  # Prevent netrc authentication, including on redirects.
        session.proxies.update(requests.utils.get_environ_proxies(link))
        session.headers['User-Agent'] = 'MDcard-public-download-acceptance/1.0'
        try:
            with session.get(link, stream=True, timeout=(30, 180), allow_redirects=True) as response:
                result['attempts'].append({'attempt': attempt, 'status': response.status_code})
                if response.status_code != 200:
                    if attempt < 3:
                        time.sleep(3)
                        continue
                    raise ValueError('Public GET did not return HTTP 200')
                chain = [*response.history, response]
                anonymous = all('Authorization' not in r.request.headers for r in chain)
                if not anonymous:
                    raise ValueError('Unexpected Authorization header')
                disposition = response.headers.get('Content-Disposition', '')
                message = Message()
                message['Content-Disposition'] = disposition
                filename = message.get_filename() or unquote(Path(urlparse(response.url).path).name)
                digest = hashlib.sha256()
                received = 0
                checkpoint = 0
                for chunk in response.iter_content(chunk_size=1024 * 1024):
                    digest.update(chunk)
                    received += len(chunk)
                    if received - checkpoint >= 64 * 1024 * 1024:
                        checkpoint = received
                        print(f'{item["platform"]}: received {received}/{item["bytes"]} bytes', flush=True)
                header_length = response.headers.get('Content-Length')
                result.update({
                    'status': response.status_code,
                    'final_host': urlparse(response.url).hostname,
                    'redirect_count': len(response.history),
                    'content_disposition': disposition,
                    'final_filename': filename,
                    'content_length': int(header_length) if header_length else None,
                    'received_bytes': received,
                    'received_sha256': digest.hexdigest(),
                    'anonymous': anonymous,
                    'seconds': round(time.time() - started, 3),
                })
                result['passed'] = (
                    filename == item['filename'] and received == item['bytes']
                    and digest.hexdigest() == item['sha256']
                    and (not header_length or int(header_length) == received)
                )
                if not result['passed']:
                    result['error'] = 'Filename, size, or full SHA256 mismatch'
                return result
        except requests.RequestException as error:
            # Exception text can contain expiring CDN query strings: retain type only.
            result['attempts'].append({'attempt': attempt, 'network_error': type(error).__name__})
            if attempt == 3:
                return {**result, 'passed': False, 'error': type(error).__name__}
            time.sleep(3)
        except ValueError as error:
            return {**result, 'passed': False, 'error': str(error)}
        finally:
            session.close()
    return {**result, 'passed': False, 'error': 'No successful complete GET'}


with ThreadPoolExecutor(max_workers=2) as executor:
    records = list(executor.map(verify, selected))
report = {
    'recorded_utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
    'page_url': captured['page_url'],
    'link_capture_utc': captured.get('recorded_utc'),
    'environment': {'os': platform.platform(), 'python': platform.python_version(), 'requests': requests.__version__},
    'method': 'Anonymous full HTTP GET, follow redirects, stream full body into SHA256; no netrc/preexisting cookies/Authorization',
    'selected_platforms': [r['platform'] for r in records],
    'full_platform_matrix': len(records) == len(releases),
    'releases': records,
    'passed': bool(records) and all(r['passed'] for r in records),
}
args.out.parent.mkdir(parents=True, exist_ok=True)
args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps(report, ensure_ascii=True), flush=True)
sys.exit(0 if report['passed'] else 1)
