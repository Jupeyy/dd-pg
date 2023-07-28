#!/usr/bin/python
import os

# make sure to have the x11 package xdotool installed on your system
print("Breakpoint hit!")
os.system("setxkbmap -option grab:break_actions")
os.system("xdotool key XF86Ungrab")
