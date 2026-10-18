# youtubedl-android usa reflexão (Jackson) e carrega bibliotecas nativas pelo nome.
-keep class com.yausername.** { *; }
-keep class com.fasterxml.jackson.** { *; }
-dontwarn com.fasterxml.jackson.**
-keep class org.apache.commons.compress.** { *; }
-dontwarn org.apache.commons.compress.**
-keep class com.adrianrabelo.ytdlp.** { *; }
