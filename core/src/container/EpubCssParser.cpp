#include "EpubCssParser.hpp"
#include "Allocator.hpp"
#include "EpubCssStyle.hpp"
#include "stringview.h"
#include <charconv>
#include <cstdlib>
#include <cstring>

namespace css {

namespace {

constexpr CssStyle parseInlineImpl(StringView styleStr) {
  CssStyle style;
  TocIterator it { styleStr, ";"};

  while (it.hasNext()) {
    StringView decl = it.next();
    size_t eqPos = decl.find(':');
    if (eqPos == decl.size()) {
      continue;
    }
    StringView name = decl[0, eqPos].trimWhitespace();
    StringView value = decl[eqPos + 1, decl.size()].trimWhitespace();

    if (name.caseCmp("text-align")) {
      if (value.caseCmp("left") || value.caseCmp("start")) {
        style.textAlign = TextAlign::Left;
      } else if (value.caseCmp("right") || value.caseCmp("end")) {
        style.textAlign = TextAlign::Right;
      } else if (value.caseCmp("center")) {
        style.textAlign = TextAlign::Center;
      } else if (value.caseCmp("justify")) {
        style.textAlign = TextAlign::Justify;
      }
    } else if (name.caseCmp("font-style")) {
      if (value.caseCmp("italic") || value.caseCmp("oblique")) {
        style.fontStyle = CssFontStyle::Italic;
      } else {
        style.fontStyle = CssFontStyle::Normal;
      }
    } else if (name.caseCmp("font-weight")) {
      if (value.caseCmp("bold") || value.caseCmp("bolder") || value.caseCmp("700") || value.caseCmp("800") || value.caseCmp("900")) {
        style.fontWeight = CssFontWeight::Bold;
      } else {
        style.fontWeight = CssFontWeight::Normal;
      }
    } else if (name.caseCmp("text-indent")) {
      // parse as float pixels for now
      float factor = 1.0f;
      if (value.endsWith("px")) {
        value = value[0, value.size() - 2];
        factor = 1.0f;
      } else if (value.endsWith("em")) {
        value = value[0, value.size() - 2];
        factor = 16.0f; // assume 1em = 16px
      } else if (value.endsWith("%")) {
        value = value[0, value.size() - 1];
        factor = 0.16f; // assume 100% = 16px
      }
      value = value.trimWhitespace();
      if (value.size() == 0) {
        continue;
      }
      float indentValue = 0.0f;
      if (std::from_chars(value.begin(), value.end(), indentValue)) {
        style.textIndent = static_cast<uint8_t>(indentValue * factor);
      }
    }
  }

  return style;
}

constexpr size_t filterComments(StringView sheet, char* buffer, size_t bufferSize) {
  auto flushBuffer = [&](StringView segment) {
    if (buffer && bufferSize >= segment.size()) {
      size_t copySize = segment.copyTo(buffer, bufferSize);
      bufferSize -= copySize;
      buffer += copySize;
    }
  };

  size_t length = 0;
  sheet.trimWhitespace();
  while (!sheet.isEmpty()) {
    size_t commentStart = sheet.find("/*");
    if (commentStart == sheet.size()) {
      length += sheet.size();
      flushBuffer(sheet);
      break;
    }
    StringView beforeComment = sheet[0, commentStart].trimWhitespace();
    length += beforeComment.size();

    flushBuffer(beforeComment);

    sheet = sheet.skip(commentStart + 2);

    size_t commentEnd = sheet.find("*/");
    if (commentEnd == sheet.size()) {
      break;
    }
    sheet = sheet.skip(commentEnd + 2);
    sheet = sheet.trimWhitespace();
  }
  return length;
}

constexpr StringView filterComments(StringView sheet, mem::Allocator& allocator) {
  // avoid duplicating the string if there are no comments
  if (sheet.find("/*") == sheet.size()) {
    return sheet;
  }
  // determine filtered length
  size_t length = filterComments(sheet, nullptr, 0);
  if (length == sheet.size()) {
    return sheet;
  }
  if (length == 0) {
    return "";
  }
  // allocate buffer and write to that
  char* buffer = allocator.bumpAlloc<char>(length);
  if (!buffer) {
    return "";
  }
  filterComments(sheet, buffer, length);
  return StringView { buffer, length };
}

constexpr size_t parseSheet(StringView sheet, CssRule* rule, size_t ruleCount) {
  size_t count = 0;
  size_t pos = 0;
  while ((pos = sheet.findAny("@{")) != sheet.size()) {
    // skip at-rule
    if (sheet[pos] == '@') {
      size_t semiPos = sheet.find(';', pos);
      if (semiPos != sheet.size()) {
        sheet = sheet.skip(semiPos + 1);
        continue;
      }
      size_t bracePos = sheet.find('{', pos);
      if (bracePos == sheet.size()) {
        break;
      }
      size_t endBracePos = sheet.find('}', bracePos);
      if (endBracePos == sheet.size()) {
        break;
      }
      sheet = sheet.skip(endBracePos + 1);
      continue;
    }

    StringView selector = sheet[0, pos].trimWhitespace();

    size_t endPos = sheet.find('}', pos);
    if (endPos == sheet.size()) {
      break;
    }

    // We only handle class selectors for now
    if (selector.size() == 0 || selector[0] != '.') {
      sheet = sheet.skip(endPos + 1);
      continue;
    }

    // Parse declarations
    StringView declarations = sheet[pos + 1, endPos].trimWhitespace();
    CssStyle style = parseInlineImpl(declarations);
    if (!style.any()) {
      sheet = sheet.skip(endPos + 1);
      continue;
    }

    if (rule && count < ruleCount) {
      rule[count] = { selector.skip(1), style };
    }
    
    sheet = sheet.skip(endPos + 1);
    count++;
  };

  return count;
}

} // namespace

CssStyle parseInline(StringView styleStr) {
  return parseInlineImpl(styleStr);
}

std::span<CssRule> parseSheet(
  StringView sheet,
  mem::Allocator& allocator
) {
  sheet = filterComments(sheet, allocator);

  size_t ruleCount = parseSheet(sheet, nullptr, 0);
  if (ruleCount == 0) {
    return {};
  }
  auto rules = allocator.subAlloc<CssRule>(ruleCount);
  if (!rules) {
    return {};
  }
  allocator.subCanary("____CssRules____");
  parseSheet(sheet, rules, ruleCount);

  auto rulesSpan = std::span<CssRule>(rules, ruleCount);

  // retain all strings
  for (auto& rule : rulesSpan) {
    rule.selector = *allocator.retain(rule.selector);
  }
  allocator.subCanary("__CssSelectors__");

  return rulesSpan;
}

CssStyle getStyle(std::span<CssRule> rules, StringView className) {
  for (const auto& rule : rules) {
    if (rule.selector == className) {
      return rule.style;
    }
  }
  return CssStyle {};
}

CssStyle getCombinedStyle(std::span<CssRule> rules, StringView classNames) {
  CssStyle combinedStyle;

  TocIterator it { classNames, " "};
  while (it.hasNext()) {
    StringView className = it.next();
    combinedStyle.merge(getStyle(rules, className));
  }
  return combinedStyle;
}


#define ASSERT_EQ(a, b) if ((a) != (b)) return false
#define ASSERT(cond) if (!(cond)) return false

constexpr bool TestCssParsing() {
  // Note: atof isn't constexpr
  CssStyle style = parseInlineImpl("text-align: center; font-style: italic; font-weight: bold;");
  ASSERT_EQ(style.textAlign, TextAlign::Center);
  ASSERT_EQ(style.textAlign, TextAlign::Center);
  ASSERT_EQ(style.fontStyle, CssFontStyle::Italic);
  ASSERT_EQ(style.fontWeight, CssFontWeight::Bold);
  // ASSERT_EQ(style.textIndent, 24.0f);

  style = parseInlineImpl("text-align: justify; font-style: normal; font-weight: normal;");
  ASSERT_EQ(style.textAlign, TextAlign::Justify);
  ASSERT_EQ(style.fontStyle, CssFontStyle::Normal);
  ASSERT_EQ(style.fontWeight, CssFontWeight::Normal);
  // ASSERT_EQ(style.textIndent, 24.0f);

  style = parseInlineImpl("text-align: right; font-style: oblique; font-weight: 700;");
  ASSERT_EQ(style.textAlign, TextAlign::Right);
  ASSERT_EQ(style.fontStyle, CssFontStyle::Italic);
  ASSERT_EQ(style.fontWeight, CssFontWeight::Bold);
  // ASSERT_EQ(style.textIndent, 24.0f);

  return true;
}

static_assert(TestCssParsing(), "CSS inline parsing tests failed");

constexpr bool TestCssTrimming() {
  {
    StringView sheet = "body { color: black; } /* comment */ h1 { font-size: 24px; }";
    char buffer[128];
    size_t length = filterComments(sheet, buffer, sizeof(buffer));
    StringView filtered = StringView { buffer, length };
    if (filtered != "body { color: black; }h1 { font-size: 24px; }") {
      return false;
    }
  }
  {
    StringView sheet = "/* full comment */";
    char buffer[128];
    size_t length = filterComments(sheet, buffer, sizeof(buffer));
    StringView filtered = StringView { buffer, length };
    if (filtered != "") {
      return false;
    }
  }
  {
    StringView sheet = "p { margin: 10px; }";
    char buffer[128];
    size_t length = filterComments(sheet, buffer, sizeof(buffer));
    StringView filtered = StringView { buffer, length };
    if (filtered != sheet) {
      return false;
    }
  }
  return true;
}

static_assert(TestCssTrimming(), "CSS comment filtering tests failed");

constexpr bool TestParseSheet() {
  {
    StringView sheet = R"css(
      .left { text-align: left; }
      .bold { font-weight: bold; }
    )css";
    size_t ruleCount = parseSheet(sheet, nullptr, 0);
    ASSERT_EQ(ruleCount, 2);
    CssRule rules[2];
    parseSheet(sheet, rules, 2);
    auto& leftRule = rules[0];
    ASSERT_EQ(leftRule.selector, "left");
    ASSERT_EQ(leftRule.style.textAlign, TextAlign::Left);
    ASSERT_EQ(leftRule.style.fontStyle.has_value(), false);
    ASSERT_EQ(leftRule.style.fontWeight.has_value(), false);
    auto& boldRule = rules[1];
    ASSERT_EQ(boldRule.selector, "bold");
    ASSERT_EQ(boldRule.style.fontWeight, CssFontWeight::Bold);
    ASSERT_EQ(boldRule.style.textAlign.has_value(), false);
    ASSERT_EQ(boldRule.style.fontStyle.has_value(), false);
  }
  {
    StringView sheet = R"css(
      /* ignored because not a class selector */
      h1 { font-size: 32px; }
      /* ignored because unknown property */
      .blub { prop: 123; }
      .calibre { font-weight: bold; }
      /* ignored because at-rule */
      @import url('styles.css');
      .headline { text-align: center; }
    )css";
    size_t ruleCount = parseSheet(sheet, nullptr, 0);
    ASSERT_EQ(ruleCount, 2);
    CssRule rules[2];
    parseSheet(sheet, rules, 2);
    auto& calibreRule = rules[0];
    ASSERT_EQ(calibreRule.selector, "calibre");
    ASSERT_EQ(calibreRule.style.fontWeight, CssFontWeight::Bold);
    ASSERT_EQ(calibreRule.style.textAlign.has_value(), false);
    ASSERT_EQ(calibreRule.style.fontStyle.has_value(), false);
    auto& headlineRule = rules[1];
    ASSERT_EQ(headlineRule.selector, "headline");
    ASSERT_EQ(headlineRule.style.textAlign, TextAlign::Center);
    ASSERT_EQ(headlineRule.style.fontStyle.has_value(), false);
    ASSERT_EQ(headlineRule.style.fontWeight.has_value(), false);
  }
  return true;
}

static_assert(TestParseSheet(), "CSS sheet parsing tests failed");

} // namespace css
