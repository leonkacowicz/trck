#!/usr/bin/env python3
#
# MIT License
#
# Copyright (c) 2026 Leon Kacowicz
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.
"""trck — a deterministic, single-file, in-repo issue tracker.

Bookkeeping is scripted; prose is hand-authored, so the tracker can't drift.
Status = the folder a Markdown file lives in; other metadata lives in index.jsonl;
SUMMARY.md is generated. Python standard library only. See README.md.
"""
from __future__ import annotations

import argparse
import ast
import json
import os
import re
import secrets
import shutil
import subprocess
import sys
import urllib.error
import urllib.request
from dataclasses import dataclass, field, fields
from datetime import datetime, timezone
from pathlib import Path
from typing import NoReturn

