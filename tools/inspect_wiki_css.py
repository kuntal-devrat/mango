import urllib.request
import re

req = urllib.request.Request(
    'https://en.wikipedia.org/wiki/Main_Page',
    headers={'User-Agent': 'Mozilla/5.0'}
)
html = urllib.request.urlopen(req).read().decode('utf-8')

css_links = re.findall(r'<link[^>]+rel=["\']stylesheet["\'][^>]+href=["\']([^"\']+)["\']', html)
print(f"Found {len(css_links)} stylesheets: {css_links}")

import html as html_lib
for link in css_links:
    link = html_lib.unescape(link)
    if link.startswith('//'):
        link = 'https:' + link
    elif link.startswith('/'):
        link = 'https://en.wikipedia.org' + link
    try:
        css = urllib.request.urlopen(urllib.request.Request(link, headers={'User-Agent': 'Mozilla/5.0'})).read().decode('utf-8')
        for m in re.finditer(r'([^{}]*(?:vector-tab|vector-menu-tabs|vector-menu-content-list)[^{}]*)\{([^}]+)\}', css, re.IGNORECASE):
            selector = m.group(1).strip()
            body = m.group(2).strip()
            if 'white-space' in body or 'nowrap' in body or 'flex-wrap' in body:
                print(f"SELECTOR: {selector}\n  BODY: {body}\n")
    except Exception as e:
        print(f"Error fetching {link}: {e}")
