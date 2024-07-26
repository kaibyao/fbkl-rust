/**
The following polyfill function is meant to run in the browser and adapted from
https://github.com/guybedford/es-module-shims
MIT License
Copyright (C) 2018-2021 Guy Bedford
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:
The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.
THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
*/

export function preloadPolyfill() {
  const relList = document.createElement('link').relList;
  if (relList && relList.supports && relList.supports('modulepreload')) {
    return;
  }

  for (const link of document.querySelectorAll('link[rel="modulepreload"]')) {
    processPreload(link);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  new MutationObserver((mutations: any) => {
    for (const mutation of mutations) {
      if (mutation.type !== 'childList') {
        continue;
      }
      for (const node of mutation.addedNodes) {
        if (node.tagName === 'LINK' && node.rel === 'modulepreload')
          processPreload(node);
      }
    }
  }).observe(document, { childList: true, subtree: true });

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  function getFetchOpts(script: any) {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const fetchOpts = {} as any;
    if (script.integrity) fetchOpts.integrity = script.integrity;
    if (script.referrerpolicy) fetchOpts.referrerPolicy = script.referrerpolicy;
    if (script.crossorigin === 'use-credentials')
      fetchOpts.credentials = 'include';
    else if (script.crossorigin === 'anonymous') fetchOpts.credentials = 'omit';
    else fetchOpts.credentials = 'same-origin';
    return fetchOpts;
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  function processPreload(link: any) {
    if (link.ep)
      // ep marker = processed
      return;
    link.ep = true;
    // prepopulate the load record
    const fetchOpts = getFetchOpts(link);
    fetch(link.href, fetchOpts);
  }
}

preloadPolyfill();
