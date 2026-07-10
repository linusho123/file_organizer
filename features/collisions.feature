Feature: Name collision handling
  When a file being moved has the same name as a file already in the
  destination type folder, the incoming file is renamed with the lowest
  available numeric suffix. Nothing is ever overwritten or lost, and every
  rename is reported in the Issues section. (PRD FR-9..FR-11)

  Scenario: A colliding file is renamed with a numeric suffix
    Given the workspace contains a subfolder named "TXT_Files" containing a file named "report.txt" with content "first"
    And the workspace contains a file named "report.txt" with content "second"
    When I run the organizer on the target
    Then the exit code is 0
    And the file "TXT_Files/report.txt" in the workspace has content "first"
    And the file "TXT_Files/report_1.txt" in the workspace has content "second"
    And the file "report.txt" does not exist in the workspace

  Scenario: The suffix increments until a free name is found
    Given the workspace contains a subfolder named "TXT_Files" containing a file named "report.txt" with content "first"
    And the workspace contains a subfolder named "TXT_Files" containing a file named "report_1.txt" with content "second"
    And the workspace contains a file named "report.txt" with content "third"
    When I run the organizer on the target
    Then the exit code is 0
    And the file "TXT_Files/report_2.txt" in the workspace has content "third"
    And the file "TXT_Files/report.txt" in the workspace has content "first"
    And the file "TXT_Files/report_1.txt" in the workspace has content "second"

  Scenario: An extensionless collision appends the suffix at the end
    Given the workspace contains a subfolder named "NO_EXTENSION_Files" containing a file named "Makefile" with content "first"
    And the workspace contains a file named "Makefile" with content "second"
    When I run the organizer on the target
    Then the exit code is 0
    And the file "NO_EXTENSION_Files/Makefile_1" in the workspace has content "second"

  Scenario: Collisions are reported in the Issues section without failing the run
    Given the workspace contains a subfolder named "TXT_Files" containing a file named "report.txt" with content "first"
    And the workspace contains a file named "report.txt" with content "second"
    When I run the organizer on the target
    Then the exit code is 0
    And the report contains "conflict:"
    And the report contains "already existed in TXT_Files"
    And the report contains "report_1.txt"
    And the report contains "Totals: 1 file moved, 0 folders created, 1 conflict, 0 errors"
