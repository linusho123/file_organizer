Feature: Keep structure mode
  With --recursive --keep-structure, subfolder contents are transported into
  the type folders with their folder organization preserved instead of
  flattened. Source folders emptied by the run are removed; undo puts
  everything back. (PRD FR-35..FR-41)

  Scenario: Pre-existing subfolders of one type are transported whole
    Given the workspace contains a nested file named "batch1/a.stori"
    And the workspace contains a nested file named "batch1/b.stori"
    And the workspace contains a nested file named "batch2/c.stori"
    And the workspace contains a nested file named "batch3/d.stori"
    When I run the organizer on the target with --recursive and --keep-structure
    Then the exit code is 0
    And the file "STORI_Files/batch1/a.stori" exists in the workspace
    And the file "STORI_Files/batch1/b.stori" exists in the workspace
    And the file "STORI_Files/batch2/c.stori" exists in the workspace
    And the file "STORI_Files/batch3/d.stori" exists in the workspace
    And the workspace does not contain a folder named "batch1"
    And the workspace does not contain a folder named "batch2"
    And the workspace does not contain a folder named "batch3"
    And the report contains "Source folders removed:"
    And the report contains "Totals: 4 files moved, 1 folder created, 0 conflicts, 0 errors"

  Scenario: Deep nesting is preserved inside the type folder
    Given the workspace contains a nested file named "a/b/c.stori"
    When I run the organizer on the target with --recursive and --keep-structure
    Then the exit code is 0
    And the file "STORI_Files/a/b/c.stori" exists in the workspace
    And the workspace does not contain a folder named "a"

  Scenario: Mixed-type subfolders are split by type with mirrored structure
    Given the workspace contains a nested file named "batch1/a.stori"
    And the workspace contains a nested file named "batch1/notes.txt"
    When I run the organizer on the target with --recursive and --keep-structure
    Then the exit code is 0
    And the file "STORI_Files/batch1/a.stori" exists in the workspace
    And the file "TXT_Files/batch1/notes.txt" exists in the workspace
    And the workspace does not contain a folder named "batch1"

  Scenario: --keep-structure requires --recursive
    Given the workspace contains a file named "notes.txt"
    When I run the organizer on the target with --keep-structure only
    Then the exit code is 2
    And stderr contains "--keep-structure requires --recursive"

  Scenario: Top-level files still land directly in their type folder
    Given the workspace contains a file named "top.md"
    And the workspace contains a nested file named "batch1/a.stori"
    When I run the organizer on the target with --recursive and --keep-structure
    Then the file "MD_Files/top.md" exists in the workspace
    And the file "STORI_Files/batch1/a.stori" exists in the workspace

  Scenario: Collisions are resolved inside the destination subfolder
    Given the workspace contains a nested file named "STORI_Files/batch1/a.stori" with content "old"
    And the workspace contains a nested file named "batch1/a.stori" with content "new"
    When I run the organizer on the target with --recursive and --keep-structure
    Then the exit code is 0
    And the file "STORI_Files/batch1/a.stori" in the workspace has content "old"
    And the file "STORI_Files/batch1/a_1.stori" in the workspace has content "new"
    And the report contains "Totals: 1 file moved, 0 folders created, 1 conflict, 0 errors"

  Scenario: Pre-existing empty folders are never removed
    Given the workspace contains an empty subfolder named "keepdir"
    And the workspace contains a nested file named "batch1/a.stori"
    When I run the organizer on the target with --recursive and --keep-structure
    Then the exit code is 0
    And the workspace contains a folder named "keepdir"
    And the workspace does not contain a folder named "batch1"

  Scenario: A folder holding a pre-existing empty subfolder is not removed
    Given the workspace contains a nested file named "batch1/a.stori"
    And the workspace contains an empty subfolder named "batch1/empty_sub"
    When I run the organizer on the target with --recursive and --keep-structure
    Then the exit code is 0
    And the file "STORI_Files/batch1/a.stori" exists in the workspace
    And the workspace contains a folder named "batch1"

  Scenario: Keep-structure dry run predicts removals and changes nothing
    Given the workspace contains a nested file named "batch1/a.stori"
    When I run the organizer on the target with --recursive, --keep-structure and --dry-run
    Then the exit code is 0
    And the workspace is unchanged
    And the report contains "DRY RUN - no changes made"
    And the report contains "batch1/a.stori  ->  STORI_Files/batch1/a.stori"
    And the report contains "Source folders removed:"

  Scenario: Undo restores transported folders exactly
    Given the workspace contains a nested file named "batch1/a.stori" with content "hello"
    And the workspace contains a nested file named "batch2/c.stori"
    When I run the organizer on the target with --recursive and --keep-structure
    And I run the organizer on the target with --undo
    Then the exit code is 0
    And the file "batch1/a.stori" in the workspace has content "hello"
    And the file "batch2/c.stori" exists in the workspace
    And the workspace does not contain a folder named "STORI_Files"
    And the file ".file_organizer_manifest.json" does not exist in the workspace
