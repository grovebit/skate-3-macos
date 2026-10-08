on run argv
    set reportPath to item 1 of argv
    set notice to "The game stopped. A diagnostic report was saved to:" & return & return & reportPath & return & return & "Nothing has been uploaded. Review the report before sharing it."
    set response to display dialog notice with title "Skate 3 Rust Engine" buttons {"Close", "Open Report"} default button "Open Report" with icon caution
    if button returned of response is "Open Report" then do shell script "/usr/bin/open -t " & quoted form of reportPath
end run
