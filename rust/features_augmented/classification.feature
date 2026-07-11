Feature: File classification by extension
  Every top-level file is moved into a subfolder named after its extension,
  uppercased, with the suffix "_Files". Extension matching is case-insensitive
  and uses the last dot; a leading dot is part of the name, not an extension
  marker. (PRD FR-5..FR-8)

  Scenario Outline: A file is moved into the folder for its extension
    Given the workspace contains a file named "<filename>"
    When I run the organizer on the target
    Then the exit code is 0
    And the file "<destination>/<filename>" exists in the workspace
    And the file "<filename>" does not exist in the workspace

    Examples:
      | filename       | destination        |
      | run01.storx    | STORX_Files        |
      | sample.mzML    | MZML_Files         |
      | calib.DMT      | DMT_Files          |
      | notes.txt      | TXT_Files          |
      | README.md      | MD_Files           |
      | archive.tar.gz | GZ_Files           |
      | Makefile       | NO_EXTENSION_Files |
      | .gitignore     | NO_EXTENSION_Files |

  Scenario: Multiple file types create one folder per extension
    Given the workspace contains a file named "run01.storx"
    And the workspace contains a file named "run02.storx"
    And the workspace contains a file named "notes.txt"
    When I run the organizer on the target
    Then the exit code is 0
    And the workspace contains a folder named "STORX_Files"
    And the workspace contains a folder named "TXT_Files"
    And the file "STORX_Files/run01.storx" exists in the workspace
    And the file "STORX_Files/run02.storx" exists in the workspace
    And the file "TXT_Files/notes.txt" exists in the workspace
    And the report contains "Totals: 3 files moved, 2 folders created, 0 conflicts, 0 errors"

  Scenario: A type folder is only created when a file classifies into it
    Given the workspace contains a file named "notes.txt"
    When I run the organizer on the target
    Then the workspace contains a folder named "TXT_Files"
    And the workspace does not contain a folder named "MD_Files"

  Scenario: Subdirectories and their contents are never touched
    Given the workspace contains a file named "notes.txt"
    And the workspace contains a subfolder named "old_backups" containing a file named "backup.dmt"
    When I run the organizer on the target
    Then the exit code is 0
    And the file "old_backups/backup.dmt" exists in the workspace
    And the workspace contains a folder named "old_backups"
    And the report contains "old_backups"
    And the report contains "(directory)"
    And the workspace does not contain a folder named "DMT_Files"

  Scenario: An existing type folder is reused, not recreated
    Given the workspace contains a subfolder named "TXT_Files" containing a file named "old.txt"
    And the workspace contains a file named "new.txt"
    When I run the organizer on the target
    Then the exit code is 0
    And the file "TXT_Files/old.txt" exists in the workspace
    And the file "TXT_Files/new.txt" exists in the workspace
    And the report contains "Totals: 1 file moved, 0 folders created, 0 conflicts, 0 errors"

  Scenario: Re-running on an already organized folder is a no-op
    Given the workspace contains a file named "run01.storx"
    And the workspace contains a file named "notes.txt"
    When I run the organizer on the target
    And I run the organizer on the target again
    Then the exit code is 0
    And the report contains "Totals: 0 files moved, 0 folders created, 0 conflicts, 0 errors"
    And the file "STORX_Files/run01.storx" exists in the workspace
    And the file "TXT_Files/notes.txt" exists in the workspace
