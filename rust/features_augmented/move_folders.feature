Feature: Move folders whole
  With --recursive --keep-structure --move-folders, a top-level subfolder
  whose files all classify to one type folder is transported whole — a
  single directory rename into the type folder — instead of being emptied
  file by file. Ineligible subfolders (mixed types, no files) fall back to
  normal keep-structure per-file handling. (PRD FR-49..FR-55)

  Scenario: A single-type subfolder is transported whole
    Given the workspace contains a nested file named "batch1/a.stori"
    And the workspace contains a nested file named "batch1/b.stori"
    And the workspace contains a nested file named "batch1/deep/c.stori"
    When I run the organizer on the target with --recursive, --keep-structure and --move-folders
    Then the exit code is 0
    And the file "STORI_Files/batch1/a.stori" exists in the workspace
    And the file "STORI_Files/batch1/b.stori" exists in the workspace
    And the file "STORI_Files/batch1/deep/c.stori" exists in the workspace
    And the workspace does not contain a folder named "batch1"
    And the report contains "Folders moved:"
    And the report contains "batch1/  ->  STORI_Files/batch1/  (3 files)"
    And the report contains "Totals: 0 files moved, 1 folder created, 0 conflicts, 0 errors"

  Scenario: A mixed-type subfolder falls back to per-file handling
    Given the workspace contains a nested file named "batch1/a.stori"
    And the workspace contains a nested file named "batch1/notes.txt"
    When I run the organizer on the target with --recursive, --keep-structure and --move-folders
    Then the exit code is 0
    And the file "STORI_Files/batch1/a.stori" exists in the workspace
    And the file "TXT_Files/batch1/notes.txt" exists in the workspace
    And the workspace does not contain a folder named "batch1"
    And the report contains "Folders moved:"
    And the report contains "Totals: 2 files moved, 2 folders created, 0 conflicts, 0 errors"

  Scenario: Top-level files still organize normally alongside a transport
    Given the workspace contains a file named "top.md"
    And the workspace contains a nested file named "batch1/a.stori"
    When I run the organizer on the target with --recursive, --keep-structure and --move-folders
    Then the exit code is 0
    And the file "MD_Files/top.md" exists in the workspace
    And the file "STORI_Files/batch1/a.stori" exists in the workspace
    And the report contains "Totals: 1 file moved, 2 folders created, 0 conflicts, 0 errors"

  Scenario: --move-folders alone is rejected
    Given the workspace contains a file named "notes.txt"
    When I run the organizer on the target with --move-folders only
    Then the exit code is 2
    And stderr contains "Error: --move-folders requires --recursive --keep-structure"

  Scenario: --move-folders with only --recursive is rejected
    Given the workspace contains a file named "notes.txt"
    When I run the organizer on the target with --recursive and --move-folders
    Then the exit code is 2
    And stderr contains "Error: --move-folders requires --recursive --keep-structure"

  Scenario: A taken destination name gets a numeric suffix on the folder
    Given the workspace contains a nested file named "STORI_Files/batch1/old.stori" with content "old"
    And the workspace contains a nested file named "batch1/new.stori" with content "new"
    When I run the organizer on the target with --recursive, --keep-structure and --move-folders
    Then the exit code is 0
    And the file "STORI_Files/batch1/old.stori" in the workspace has content "old"
    And the file "STORI_Files/batch1_1/new.stori" in the workspace has content "new"
    And the workspace does not contain a folder named "batch1"
    And the report contains "conflict:"
    And the report contains "already existed in STORI_Files; moved as"
    And the report contains "Totals: 0 files moved, 0 folders created, 1 conflict, 0 errors"

  Scenario: Dry run previews the transport and changes nothing
    Given the workspace contains a nested file named "batch1/a.stori"
    When I run the organizer on the target with --recursive, --keep-structure, --move-folders and --dry-run
    Then the exit code is 0
    And the workspace is unchanged
    And the report contains "DRY RUN - no changes made"
    And the report contains "batch1/  ->  STORI_Files/batch1/  (1 file)"

  Scenario: A nested manifest travels with the folder
    Given the workspace contains a nested file named "batch1/a.stori"
    And the workspace contains a nested file named "batch1/.file_organizer_manifest.json"
    When I run the organizer on the target with --recursive, --keep-structure and --move-folders
    Then the exit code is 0
    And the file "STORI_Files/batch1/a.stori" exists in the workspace
    And the file "STORI_Files/batch1/.file_organizer_manifest.json" exists in the workspace
    And the report contains "batch1/  ->  STORI_Files/batch1/  (2 files)"

  Scenario: Empty nested subfolders travel with the folder
    Given the workspace contains a nested file named "batch1/a.stori"
    And the workspace contains an empty subfolder named "batch1/empty_sub"
    When I run the organizer on the target with --recursive, --keep-structure and --move-folders
    Then the exit code is 0
    And the file "STORI_Files/batch1/a.stori" exists in the workspace
    And the workspace contains a folder named "STORI_Files/batch1/empty_sub"
    And the workspace does not contain a folder named "batch1"

  Scenario: A second run over the organized folder is a no-op
    Given the workspace contains a nested file named "batch1/a.stori"
    When I run the organizer on the target with --recursive, --keep-structure and --move-folders
    And I run the organizer on the target with --recursive, --keep-structure and --move-folders
    Then the exit code is 0
    And the file "STORI_Files/batch1/a.stori" exists in the workspace
    And the report contains "Totals: 0 files moved, 0 folders created, 0 conflicts, 0 errors"

  Scenario: Undo restores a transported folder whole
    Given the workspace contains a nested file named "batch1/a.stori" with content "hello"
    When I run the organizer on the target with --recursive, --keep-structure and --move-folders
    And I run the organizer on the target with --undo
    Then the exit code is 0
    And the file "batch1/a.stori" in the workspace has content "hello"
    And the workspace does not contain a folder named "STORI_Files"
    And the file ".file_organizer_manifest.json" does not exist in the workspace
    And the report contains "Folders restored:"
    And the report contains "STORI_Files/batch1/  ->  batch1/"

  Scenario: Undo resolves a retaken folder name with a numeric suffix
    Given the workspace contains a nested file named "batch1/a.stori" with content "original"
    When I run the organizer on the target with --recursive, --keep-structure and --move-folders
    And the workspace gains a file named "batch1/marker.txt" with content "newcomer"
    And I run the organizer on the target with --undo
    Then the exit code is 0
    And the file "batch1_1/a.stori" in the workspace has content "original"
    And the file "batch1/marker.txt" in the workspace has content "newcomer"
    And the report contains "conflict:"
    And the report contains "already existed; restored as"

  Scenario: Undo reports a missing transported folder as an error and keeps the manifest
    Given the workspace contains a nested file named "batch1/a.stori"
    When I run the organizer on the target with --recursive, --keep-structure and --move-folders
    And the file "STORI_Files/batch1/a.stori" is deleted from the workspace
    And the folder "STORI_Files/batch1" is deleted from the workspace
    And I run the organizer on the target with --undo
    Then the exit code is 1
    And the report contains "error: could not restore"
    And the report contains "folder not found"
    And the file ".file_organizer_manifest.json" exists in the workspace
