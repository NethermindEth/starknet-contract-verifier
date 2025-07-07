# UX Improvement Plan - Starknet Contract Verifier

## Overview

This document outlines a comprehensive plan to improve the user experience of the Starknet Contract Verifier tool. The improvements are categorized by priority and focus on reducing friction for new users while maintaining power for experienced developers.

## 🔥 HIGH PRIORITY - Critical User Experience Issues

### 1. Improve Error Messages & Guidance

- **Current Issue**: Generic error messages like "Contract: X is not defined in the manifest file"
- **Problem**: Users don't know what went wrong or how to fix it
- **Solution**: Add context-aware error messages with suggested actions and examples
- **Implementation**:
  - Enhanced error types with suggestions in `src/errors.rs`
  - Better error formatting in `src/main.rs`
  - Add "Did you mean..." suggestions for common mistakes

### 2. Add Progress Indicators

- **Current Issue**: Silent operation until completion, unclear when polling
- **Problem**: Users don't know if tool is working or stuck
- **Solution**: Real-time progress bars, spinner animations, clearer polling feedback
- **Implementation**:
  - Add `indicatif` crate for progress bars
  - Spinner during file processing and API calls
  - Better polling feedback with time estimates

### 3. Enhance Status Checking UX

- **Current Issue**: Users must manually check status with separate command
- **Problem**: Poor workflow, easy to lose track of job IDs
- **Solution**: Auto-watch mode, save job IDs locally, provide direct links
- **Implementation**:
  - `--watch` flag to auto-poll until completion
  - Local job history file in `~/.starknet-verifier/`
  - Direct clickable links to Voyager

## 🔧 MEDIUM PRIORITY - Usability Improvements

### 4. Better License Handling

- **Current Issue**: Complex license parsing with unclear error messages
- **Problem**: Users confused about license format
- **Solution**: License picker with common options, better validation messages
- **Implementation**:
  - Interactive license picker
  - Better SPDX validation
  - License detection from common patterns

### 5. Workspace Project Support

- **Current Issue**: Package selection is confusing for workspace projects
- **Problem**: Users don't understand when `--package` is required
- **Solution**: Auto-detect workspace structure, list available packages
- **Implementation**:
  - Auto-detect workspace vs single package
  - List available packages when `--package` is required
  - Better error messages for workspace projects

### 6. Dry-run Output Enhancement

- **Current Issue**: Basic file listing without context
- **Problem**: Users can't verify what will be submitted
- **Solution**: Structured preview with file sizes, package info, validation warnings
- **Implementation**:
  - Structured table output
  - File size information
  - Validation warnings before submission

## ✨ LOW PRIORITY - Nice-to-Have Features

### 7. Command Shortcuts & Aliases

- **Current Issue**: Long command names
- **Problem**: Verbose for frequent users
- **Solution**: Short aliases like `verify`, `check`, common flag shortcuts
- **Implementation**:
  - Add command aliases in `clap` configuration
  - Short flags for common options
  - Shell completion scripts

### 8. Verification History

- **Current Issue**: No history tracking
- **Problem**: Users lose track of previous verifications
- **Solution**: Local history file, `--list` command to show past jobs
- **Implementation**:
  - History storage in `~/.starknet-verifier/history.json`
  - `list` subcommand to show past verifications
  - Filter and search capabilities

### 9. Smart Defaults

- **Current Issue**: Users must specify common options
- **Problem**: Repetitive for standard use cases
- **Solution**: Remember preferences, detect common patterns
- **Implementation**:
  - Learn from user patterns
  - Project-specific defaults
  - Network preference memory

### 10. Better Success Feedback

- **Current Issue**: Just prints Voyager URL
- **Problem**: Users might miss the success message
- **Solution**: Colored output, ASCII art success banner, clipboard integration
- **Implementation**:
  - Colored success messages
  - ASCII art celebration
  - Copy URL to clipboard option

## 🎯 Implementation Priority

### Phase 1 (Immediate Impact)

**Timeline**: 1-2 weeks

- Enhanced error messages with suggestions
- Progress indicators and better feedback
- Interactive prompts for missing arguments

### Phase 2 (Workflow Improvements)

**Timeline**: 2-3 weeks

- Auto-watch mode for status checking
- Better dry-run output
- Workspace project auto-detection

### Phase 3 (Power User Features)

**Timeline**: 3-4 weeks

- Configuration file support
- Verification history
- Command shortcuts

## Technical Implementation Notes

### New Dependencies

- `indicatif` - Progress bars and spinners
- `dialoguer` - Interactive prompts
- `serde` - Configuration file handling
- `dirs` - Home directory detection
- `clipboard` - Copy to clipboard functionality

### File Structure Changes

```
src/
├── config.rs         # Configuration management
├── history.rs        # Verification history
├── interactive.rs    # Interactive prompts
├── progress.rs       # Progress indicators
└── ui.rs            # UI utilities and formatting
```

### Configuration File Format

```toml
[network]
default = "mainnet"
mainnet_public = "https://api.voyager.online/beta"
mainnet_private = "https://voyager.online"

[defaults]
license = "MIT"
watch = true
lock_file = false

[ui]
colored_output = true
progress_bars = true
```

## Success Metrics

- **Reduced Error Rate**: Fewer failed verifications due to user errors
- **Faster Time to Success**: Reduced time from installation to first successful verification
- **User Satisfaction**: Positive feedback on CLI usability
- **Support Load**: Reduced support questions about basic usage

## Conclusion

This plan focuses on addressing the most common pain points that users face when using the Starknet Contract Verifier. By implementing these improvements in phases, we can significantly enhance the user experience while maintaining the tool's reliability and power.
